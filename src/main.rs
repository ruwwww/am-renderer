//! CLI interface and XML-to-model converter for the Alight Motion renderer.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use am_renderer::model::effect::*;
use am_renderer::model::*;
use am_renderer::parser::*;
use am_renderer::render::parse_hex_color;

#[derive(Parser, Debug)]
#[command(
    name = "am-renderer",
    version = "0.1.0",
    about = "Headless renderer for Alight Motion XML projects"
)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Render a project (single frame or full sequence/video).
    Render {
        /// Path to the Alight Motion XML file.
        #[arg(short, long)]
        input: PathBuf,

        /// Path to the directory containing imported media assets.
        #[arg(short, long)]
        assets: PathBuf,

        /// Path for the output (directory for png sequence, file for mp4).
        #[arg(short, long)]
        output: PathBuf,

        /// Output format (png or mp4). If not specified, auto-detects from output path.
        #[arg(long)]
        format: Option<Format>,

        /// Render a single frame instead of the full project.
        #[arg(long)]
        frame: Option<u32>,

        /// Dump the compiled render/effect graph of evaluated frames.
        #[arg(long)]
        dump_graph: bool,
    },
    /// Print metadata information about the project.
    Info {
        /// Path to the Alight Motion XML file.
        #[arg(short, long)]
        input: PathBuf,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Png,
    Mp4,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    match args.command {
        Commands::Info { input } => {
            let xml_scene = am_renderer::parser::parse_xml(&input)?;
            let project = convert_project(&xml_scene)?;
            println!("Project: {}", project.title.as_deref().unwrap_or("Untitled"));
            println!("Canvas Size: {}x{}", project.width, project.height);
            println!("Export Size: {}x{}", project.export_width, project.export_height);
            println!("Duration: {:.2}s", project.duration_secs());
            println!("FPS: {}", project.fps);
            println!("Total Frames: {}", project.total_frames());
            print_distinct_media_inputs(&project);
            println!("Layers: {}", project.layers.len());
            for (idx, l) in project.layers.iter().enumerate() {
                println!(
                    "  [{}] Layer: {} (id={}, times: {}ms - {}ms, fill: {:?}, blend: {:?})",
                    idx,
                    l.label.as_deref().unwrap_or("unnamed"),
                    l.id,
                    l.start_time,
                    l.end_time,
                    l.fill_type,
                    l.blend_mode
                );
            }
        }
        Commands::Render {
            input,
            assets,
            output,
            format,
            frame,
            dump_graph,
        } => {
            let xml_scene = am_renderer::parser::parse_xml(&input)?;
            let project = convert_project(&xml_scene)?;

            let fmt = match format {
                Some(f) => f,
                None => {
                    if let Some(ext) = output.extension() {
                        if ext.to_string_lossy().eq_ignore_ascii_case("mp4") {
                            Format::Mp4
                        } else {
                            Format::Png
                        }
                    } else {
                        Format::Png
                    }
                }
            };

            if let Some(f) = frame {
                let time_secs = f as f32 / project.fps;
                let resolved = am_renderer::eval::timeline::evaluate(&project, time_secs);
                if dump_graph {
                    print_render_graph(&resolved, f, time_secs);
                }
                let mut cache = am_renderer::render::compositor::ImageCache::new();
                let img = am_renderer::render::compositor::render_scene(&resolved, &mut cache, &assets)?;

                let out_dir = if fmt == Format::Png {
                    output
                } else {
                    output.parent().unwrap_or(&output).to_path_buf()
                };
                am_renderer::export::png::export_frame(&img, &out_dir, f)?;
                println!("Successfully rendered frame {} to {}", f, out_dir.display());
            } else {
                if dump_graph {
                    println!("Dumping graph for frame 0 (start of sequence):");
                    let resolved = am_renderer::eval::timeline::evaluate(&project, 0.0);
                    print_render_graph(&resolved, 0, 0.0);
                }
                match fmt {
                    Format::Png => {
                        am_renderer::export::png::export_sequence(&project, &assets, &output, None, None)?;
                        println!("Successfully rendered sequence to {}", output.display());
                    }
                    Format::Mp4 => {
                        // Create a temporary directory in workspace for frame rendering
                        let temp_dir = std::env::current_dir()?.join(".temp_frames");
                        if temp_dir.exists() {
                            std::fs::remove_dir_all(&temp_dir)?;
                        }
                        std::fs::create_dir_all(&temp_dir)?;

                        println!("Rendering frames...");
                        am_renderer::export::png::export_sequence(&project, &assets, &temp_dir, None, None)?;

                        println!("Stitching video using FFmpeg...");
                        am_renderer::export::video::export_mp4(
                            &temp_dir,
                            &output,
                            project.fps as u32,
                            project.width,
                            project.height,
                        )?;

                        // Cleanup temp frames
                        std::fs::remove_dir_all(&temp_dir)?;
                        println!("Successfully rendered video to {}", output.display());
                    }
                }
            }
        }
    }

    Ok(())
}

fn print_distinct_media_inputs(project: &Project) {
    use std::collections::HashSet;

    let mut referenced_uris = HashSet::new();
    for layer in &project.layers {
        if layer.fill_type == FillType::Media {
            if let Some(ref uri) = layer.fill_image {
                referenced_uris.insert(uri.clone());
            }
        }
    }
    for track in &project.audio_tracks {
        if let Some(ref uri) = track.src {
            referenced_uris.insert(uri.clone());
        }
    }
    for m in &project.media {
        referenced_uris.insert(m.uri.clone());
    }

    if referenced_uris.is_empty() {
        println!("Distinct Media Inputs Required: None");
        return;
    }

    println!("Distinct Media Inputs Required ({}):", referenced_uris.len());
    let mut sorted_uris: Vec<String> = referenced_uris.into_iter().collect();
    sorted_uris.sort();

    for uri in sorted_uris {
        let metadata = project.media.iter().find(|m| m.uri == uri);
        let label = metadata.and_then(|m| m.title.as_deref())
            .or_else(|| metadata.and_then(|m| m.filename.as_deref()))
            .unwrap_or_else(|| {
                uri.rsplit('/')
                    .next()
                    .unwrap_or(&uri)
            });

        let mime = metadata.and_then(|m| m.mime_type.as_deref())
            .unwrap_or_else(|| {
                if uri.to_lowercase().ends_with(".mp3") || uri.to_lowercase().ends_with(".wav") {
                    "audio/unknown"
                } else {
                    "image/unknown"
                }
            });

        let dim_str = if let (Some(w), Some(h)) = (metadata.and_then(|m| m.width), metadata.and_then(|m| m.height)) {
            format!(" [{}x{}]", w, h)
        } else {
            "".to_string()
        };

        println!("  - {} (URI: {}, type: {}{})", label, uri, mime, dim_str);
    }
}

fn print_render_graph(scene: &am_renderer::eval::ResolvedScene, frame_num: u32, time_secs: f32) {
    println!("=== Render Graph for Frame {} (at {:.3}s) ===", frame_num, time_secs);
    println!("Canvas size: {}x{}", scene.width, scene.height);
    println!("Background color: {:?}", scene.bg_color);
    println!("Layers (bottom-to-top):");
    for (idx, layer) in scene.layers.iter().enumerate() {
        println!(
            "  [{}] Layer: {} (id={})",
            idx,
            layer.label.as_deref().unwrap_or("unnamed"),
            layer.id
        );
        println!("      Size: {}x{}", layer.size[0], layer.size[1]);
        println!("      Fill Type: {:?}", layer.fill_type);
        if let Some(ref img) = layer.fill_image {
            println!("      Fill Image URI: {}", img);
        }
        println!("      Position: {:?}", layer.location);
        println!("      Scale: {:?}", layer.scale);
        println!("      Rotation: {:.2}°", layer.rotation);
        println!("      Opacity: {:.2}", layer.opacity);
        println!("      Blend Mode: {:?}", layer.blend_mode);
        if layer.effects.is_empty() {
            println!("      Effects: None");
        } else {
            println!("      Effects Chain:");
            for (e_idx, effect) in layer.effects.iter().enumerate() {
                println!(
                    "        - ({}.{}) {:?} (locally applied: {})",
                    idx,
                    e_idx,
                    effect.effect_type,
                    effect.locally_applied
                );
            }
        }
    }
    println!("=============================================");
}

// ---------------------------------------------------------------------------
// XML-to-Model Conversion Helpers
// ---------------------------------------------------------------------------

fn convert_project(xml: &XmlScene) -> Result<Project> {
    let width = xml.width.parse().context("invalid scene width")?;
    let height = xml.height.parse().context("invalid scene height")?;
    let export_width = xml
        .export_width
        .as_deref()
        .and_then(|w| w.parse().ok())
        .unwrap_or(width);
    let export_height = xml
        .export_height
        .as_deref()
        .and_then(|h| h.parse().ok())
        .unwrap_or(height);

    let bg_color = xml
        .bgcolor
        .as_deref()
        .map(parse_hex_color)
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);

    let total_time = xml.total_time.parse().context("invalid totalTime")?;
    let fps = xml.fps.parse().context("invalid fps")?;

    let media = xml.media.iter().map(convert_media).collect();
    let audio_tracks = xml.audio.iter().map(convert_audio).collect();

    let mut layers = Vec::new();
    for shape in &xml.shapes {
        let id = shape.id.parse().unwrap_or(0);
        let label = shape.label.clone();
        let start_time = shape.start_time.parse().unwrap_or(0.0);
        let end_time = shape.end_time.parse().unwrap_or(0.0);
        let hidden = shape
            .hidden
            .as_deref()
            .map(|v| v == "true")
            .unwrap_or(false);

        let transform = if let Some(ref t) = shape.transform {
            LayerTransform {
                location: t
                    .location
                    .as_ref()
                    .map(|l| convert_animated_vec3(l, [0.0, 0.0, 0.0]))
                    .unwrap_or(Animated::Static([0.0, 0.0, 0.0])),
                scale: t
                    .scale
                    .as_ref()
                    .map(|s| convert_animated_vec2(s, [1.0, 1.0]))
                    .unwrap_or(Animated::Static([1.0, 1.0])),
                rotation: t
                    .rotation
                    .as_ref()
                    .map(|r| convert_animated_float(r, 0.0))
                    .unwrap_or(Animated::Static(0.0)),
                opacity: t
                    .opacity
                    .as_ref()
                    .map(|o| convert_animated_float(o, 1.0))
                    .unwrap_or(Animated::Static(1.0)),
            }
        } else {
            LayerTransform {
                location: Animated::Static([0.0, 0.0, 0.0]),
                scale: Animated::Static([1.0, 1.0]),
                rotation: Animated::Static(0.0),
                opacity: Animated::Static(1.0),
            }
        };

        let fill_type = match shape.fill_type.as_deref() {
            Some("media") => FillType::Media,
            Some("color") => FillType::Color,
            Some("gradient") => FillType::Gradient,
            _ => {
                if shape.fill_image.is_some() {
                    FillType::Media
                } else if shape.fill_color.is_some() {
                    FillType::Color
                } else if shape.gradient.is_some() {
                    FillType::Gradient
                } else {
                    FillType::None
                }
            }
        };

        let fill_image = shape.fill_image.clone();
        let fill_color = shape
            .fill_color
            .as_ref()
            .map(|fc| parse_hex_color(&fc.value))
            .unwrap_or([1.0, 1.0, 1.0, 1.0]);

        let gradient = shape.gradient.as_ref().map(|g| {
            let start = g
                .start
                .as_deref()
                .map(|s| parse_vec2(s, [0.0, 0.0]))
                .unwrap_or([0.0, 0.0]);
            let end = g
                .end
                .as_deref()
                .map(|s| parse_vec2(s, [1.0, 1.0]))
                .unwrap_or([1.0, 1.0]);
            let mut stops = Vec::new();
            if let Some(ref sc) = g.start_color {
                stops.push(GradientStop {
                    position: 0.0,
                    color: parse_hex_color(sc),
                });
            }
            if let Some(ref ec) = g.end_color {
                stops.push(GradientStop {
                    position: 1.0,
                    color: parse_hex_color(ec),
                });
            }
            Gradient { start, end, stops }
        });

        let blend_mode = match shape.blending.as_deref() {
            Some("multiply") => BlendMode::Multiply,
            Some("screen") => BlendMode::Screen,
            Some("overlay") => BlendMode::Overlay,
            Some("darken") => BlendMode::Darken,
            Some("lighten") => BlendMode::Lighten,
            Some("subtract") => BlendMode::Subtract,
            Some("add") => BlendMode::Add,
            _ => BlendMode::Normal,
        };

        let size = shape
            .properties
            .iter()
            .find(|p| p.name == "size")
            .and_then(|p| p.value.as_deref())
            .map(|v| parse_vec2(v, [100.0, 100.0]))
            .unwrap_or([100.0, 100.0]);

        let effects = shape.effects.iter().map(convert_effect).collect();

        layers.push(Layer {
            id,
            label,
            start_time,
            end_time,
            hidden,
            transform,
            fill_type,
            fill_image,
            fill_color,
            gradient,
            blend_mode,
            effects,
            size,
        });
    }

    Ok(Project {
        title: xml.title.clone(),
        width,
        height,
        export_width,
        export_height,
        bg_color,
        total_time,
        fps,
        media,
        audio_tracks,
        layers,
    })
}

fn convert_media(xml: &XmlMedia) -> MediaRef {
    MediaRef {
        uri: xml.uri.clone(),
        filename: xml.filename.clone(),
        title: xml.title.clone(),
        mime_type: xml.r#type.clone(),
        width: xml.width.as_deref().and_then(|w| w.parse().ok()),
        height: xml.height.as_deref().and_then(|h| h.parse().ok()),
    }
}

fn convert_audio(xml: &XmlAudio) -> AudioTrack {
    AudioTrack {
        id: xml.id.parse().unwrap_or(0),
        label: xml.label.clone(),
        start_time: xml.start_time.parse().unwrap_or(0.0),
        end_time: xml.end_time.parse().unwrap_or(0.0),
        src: xml.src.clone(),
    }
}

fn parse_easing(s: Option<&str>) -> EasingType {
    let s = match s {
        Some(val) => val.trim(),
        None => return EasingType::Linear,
    };
    if s.is_empty() {
        return EasingType::Linear;
    }
    let parts: Vec<&str> = s.split_whitespace().collect();
    let start_idx = if parts.first() == Some(&"local") { 1 } else { 0 };
    if parts.get(start_idx) == Some(&"cubicBezier") && parts.len() >= start_idx + 5 {
        let x1 = parts[start_idx + 1].parse().unwrap_or(0.0);
        let y1 = parts[start_idx + 2].parse().unwrap_or(0.0);
        let x2 = parts[start_idx + 3].parse().unwrap_or(0.0);
        let y2 = parts[start_idx + 4].parse().unwrap_or(0.0);
        EasingType::CubicBezier(x1, y1, x2, y2)
    } else {
        EasingType::Linear
    }
}

fn parse_vec2(s: &str, default: [f32; 2]) -> [f32; 2] {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() >= 2 {
        let x = parts[0].trim().parse().unwrap_or(default[0]);
        let y = parts[1].trim().parse().unwrap_or(default[1]);
        [x, y]
    } else {
        default
    }
}

fn parse_vec3(s: &str, default: [f32; 3]) -> [f32; 3] {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() >= 3 {
        let x = parts[0].trim().parse().unwrap_or(default[0]);
        let y = parts[1].trim().parse().unwrap_or(default[1]);
        let z = parts[2].trim().parse().unwrap_or(default[2]);
        [x, y, z]
    } else if parts.len() == 2 {
        let x = parts[0].trim().parse().unwrap_or(default[0]);
        let y = parts[1].trim().parse().unwrap_or(default[1]);
        [x, y, default[2]]
    } else {
        default
    }
}

fn convert_animated_float(xml: &XmlAnimatedFloat, default_val: f32) -> Animated<f32> {
    if !xml.keyframes.is_empty() {
        let mut kfs: Vec<Keyframe<f32>> = xml
            .keyframes
            .iter()
            .map(|kf| {
                let t = kf.t.parse().unwrap_or(0.0);
                let value = kf.v.parse().unwrap_or(default_val);
                let easing = parse_easing(kf.e.as_deref());
                Keyframe { t, value, easing }
            })
            .collect();
        kfs.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
        Animated::Keyframed(kfs)
    } else {
        let val = xml
            .value
            .as_deref()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default_val);
        Animated::Static(val)
    }
}

fn convert_animated_vec2(xml: &XmlAnimatedVec2, default_val: [f32; 2]) -> Animated<[f32; 2]> {
    if !xml.keyframes.is_empty() {
        let mut kfs: Vec<Keyframe<[f32; 2]>> = xml
            .keyframes
            .iter()
            .map(|kf| {
                let t = kf.t.parse().unwrap_or(0.0);
                let value = parse_vec2(&kf.v, default_val);
                let easing = parse_easing(kf.e.as_deref());
                Keyframe { t, value, easing }
            })
            .collect();
        kfs.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
        Animated::Keyframed(kfs)
    } else {
        let val = xml
            .value
            .as_deref()
            .map(|v| parse_vec2(v, default_val))
            .unwrap_or(default_val);
        Animated::Static(val)
    }
}

fn convert_animated_vec3(xml: &XmlAnimatedVec3, default_val: [f32; 3]) -> Animated<[f32; 3]> {
    if !xml.keyframes.is_empty() {
        let mut kfs: Vec<Keyframe<[f32; 3]>> = xml
            .keyframes
            .iter()
            .map(|kf| {
                let t = kf.t.parse().unwrap_or(0.0);
                let value = parse_vec3(&kf.v, default_val);
                let easing = parse_easing(kf.e.as_deref());
                Keyframe { t, value, easing }
            })
            .collect();
        kfs.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
        Animated::Keyframed(kfs)
    } else {
        let val = xml
            .value
            .as_deref()
            .map(|v| parse_vec3(v, default_val))
            .unwrap_or(default_val);
        Animated::Static(val)
    }
}

fn get_prop_animated_float(
    properties: &[XmlProperty],
    name: &str,
    default_val: f32,
) -> Animated<f32> {
    if let Some(p) = properties.iter().find(|p| p.name == name) {
        if !p.keyframes.is_empty() {
            let mut kfs: Vec<Keyframe<f32>> = p
                .keyframes
                .iter()
                .map(|kf| {
                    let t = kf.t.parse().unwrap_or(0.0);
                    let value = kf.v.parse().unwrap_or(default_val);
                    let easing = parse_easing(kf.e.as_deref());
                    Keyframe { t, value, easing }
                })
                .collect();
            kfs.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
            Animated::Keyframed(kfs)
        } else {
            let val = p
                .value
                .as_deref()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default_val);
            Animated::Static(val)
        }
    } else {
        Animated::Static(default_val)
    }
}

fn get_prop_float(properties: &[XmlProperty], name: &str, default_val: f32) -> f32 {
    properties
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.value.as_deref())
        .and_then(|v| v.parse().ok())
        .unwrap_or(default_val)
}

fn get_prop_bool(properties: &[XmlProperty], name: &str, default_val: bool) -> bool {
    properties
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.value.as_deref())
        .map(|v| v == "true")
        .unwrap_or(default_val)
}

fn get_prop_vec2(properties: &[XmlProperty], name: &str, default_val: [f32; 2]) -> [f32; 2] {
    properties
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.value.as_deref())
        .map(|v| parse_vec2(v, default_val))
        .unwrap_or(default_val)
}

fn get_prop_color3(properties: &[XmlProperty], name: &str, default_val: [f32; 3]) -> [f32; 3] {
    properties
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.value.as_deref())
        .map(|v| {
            let rgba = parse_hex_color(v);
            [rgba[0], rgba[1], rgba[2]]
        })
        .unwrap_or(default_val)
}

fn get_prop_color4(properties: &[XmlProperty], name: &str, default_val: [f32; 4]) -> [f32; 4] {
    properties
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.value.as_deref())
        .map(|v| parse_hex_color(v))
        .unwrap_or(default_val)
}

fn convert_effect(xml: &XmlEffect) -> Effect {
    let locally_applied = xml
        .locally_applied
        .as_deref()
        .map(|v| v == "true")
        .unwrap_or(true);
    let props = &xml.properties;

    let effect_type = match xml.id.as_str() {
        "com.alightcreative.effects.oscillate3" => EffectType::Oscillate(OscillateParams {
            angle: get_prop_animated_float(props, "angle", 0.0),
            freq: get_prop_animated_float(props, "freq", 1.0),
            mag: get_prop_animated_float(props, "mag", 10.0),
            direction: get_prop_float(props, "direction", 0.0) as i32,
            osc_type: get_prop_float(props, "osc_type", 0.0) as i32,
            phase: get_prop_float(props, "phase", 0.0),
        }),
        "com.alightcreative.effects.swing2" => EffectType::Swing(SwingParams {
            a1: get_prop_animated_float(props, "a1", 15.0),
            a2: get_prop_animated_float(props, "a2", 0.0),
            freq: get_prop_animated_float(props, "freq", 1.0),
        }),
        "com.alightcreative.effects.randomdisplace" => {
            EffectType::RandomDisplace(RandomDisplaceParams {
                evolution: get_prop_animated_float(props, "evolution", 0.0),
                mag: get_prop_animated_float(props, "mag", 10.0),
                seed: get_prop_float(props, "seed", 0.0),
                scatter: get_prop_float(props, "scatter", 1.0),
            })
        }
        "com.alightcreative.effects.motionblur3" => EffectType::MotionBlur(MotionBlurParams {
            tune: get_prop_animated_float(props, "tune", 1.0),
            use_pos: get_prop_bool(props, "use_pos", true),
            use_scale: get_prop_bool(props, "use_scale", true),
            use_angle: get_prop_bool(props, "use_angle", true),
        }),
        "com.alightcreative.effects.blink" => EffectType::Blink(BlinkParams {
            freq: get_prop_animated_float(props, "freq", 2.0),
        }),
        "com.alightcreative.effects.fade" => EffectType::Fade(FadeParams {
            in_time: get_prop_float(props, "in_time", 200.0),
            out_time: get_prop_float(props, "out_time", 200.0),
        }),
        "com.alightcreative.effects.tile" => EffectType::Tile(TileParams {
            mirror: get_prop_bool(props, "mirror", false),
            scale: get_prop_float(props, "scale", 1.0),
            phase: get_prop_float(props, "phase", 0.0),
            vert_offset: get_prop_bool(props, "vert_offset", false),
            angle: get_prop_float(props, "angle", 0.0),
        }),
        "com.alightcreative.effects.exposure" => EffectType::Exposure(ExposureParams {
            exposure: get_prop_animated_float(props, "exposure", 0.0),
            gamma: get_prop_animated_float(props, "gamma", 1.0),
            offset: get_prop_float(props, "offset", 0.0),
        }),
        "com.alightcreative.effects.brightnesscontrast" => {
            EffectType::BrightnessContrast(BrightnessContrastParams {
                brightness: get_prop_float(props, "brightness", 0.0),
                contrast: get_prop_float(props, "contrast", 0.0),
            })
        }
        "com.alightcreative.effects.saturationvibrance" => {
            EffectType::SaturationVibrance(SaturationVibranceParams {
                saturation: get_prop_float(props, "saturation", 0.0),
                vibrance: get_prop_float(props, "vibrance", 0.0),
            })
        }
        "com.alightcreative.effects.colortint" => EffectType::ColorTint(ColorTintParams {
            tint: get_prop_color3(props, "tint", [1.0, 1.0, 1.0]),
        }),
        "com.alightcreative.effects.highlightshadow" => {
            EffectType::HighlightShadow(HighlightShadowParams {
                highlights: get_prop_float(props, "highlights", 0.0),
                shadows: get_prop_float(props, "shadows", 0.0),
            })
        }
        "com.alightcreative.effects.vignette" => EffectType::Vignette(VignetteParams {
            feather: get_prop_float(props, "feather", 0.5),
            roundness: get_prop_float(props, "roundness", 1.0),
            scale: get_prop_float(props, "scale", 1.0),
            strength: get_prop_float(props, "strength", 0.5),
            tint: get_prop_float(props, "tint", 0.0),
        }),
        "com.alightcreative.effects.sharpen" => EffectType::Sharpen(SharpenParams {
            radius: get_prop_float(props, "radius", 1.0),
            strength: get_prop_float(props, "strength", 0.5),
        }),
        "com.alightcreative.effects.gaussianblur" => EffectType::GaussianBlur(GaussianBlurParams {
            radius: get_prop_float(props, "radius", 5.0),
        }),
        "com.alightcreative.effects.lensblur" => EffectType::LensBlur(LensBlurParams {
            radius: get_prop_float(props, "radius", 5.0),
            strength: get_prop_float(props, "strength", 1.0),
        }),
        "com.alightcreative.effects.gradientoverlay" => {
            EffectType::GradientOverlay(GradientOverlayParams {
                alpha: get_prop_float(props, "alpha", 1.0),
                color1: get_prop_color4(props, "color1", [0.0, 0.0, 0.0, 1.0]),
                color2: get_prop_color4(props, "color2", [1.0, 1.0, 1.0, 1.0]),
                offset: get_prop_vec2(props, "offset", [0.0, 0.0]),
                scale: get_prop_float(props, "scale", 1.0),
            })
        }
        "com.alightcreative.effects.lift" => EffectType::Lift,
        "com.alightcreative.effects.lumakey" => EffectType::LumaKey(LumaKeyParams {
            low_threshold: get_prop_animated_float(props, "low_threshold", 0.0),
            high_threshold: get_prop_animated_float(props, "high_threshold", 1.0),
        }),
        other => EffectType::Unknown(other.to_string()),
    };

    Effect {
        effect_type,
        locally_applied,
    }
}
