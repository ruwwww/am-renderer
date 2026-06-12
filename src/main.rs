//! CLI interface and XML-to-model converter for the Alight Motion renderer.

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use am_renderer::config::Config;
use am_renderer::model::*;
use am_renderer::parser::*;
use am_renderer::render::debug_effects::render_effects_debug;

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

        /// Auto-pair unmatched template media URIs to available source assets virtually.
        /// Use --no-auto-pair to disable.
        #[arg(long = "auto-pair", default_missing_value = "true", default_value_t = true, num_args = 0..=1, require_equals = false)]
        auto_pair: bool,

        /// Render with canvas zoomed out, borders shown, and element labels overlayed.
        #[arg(long)]
        debug_layout: bool,

        /// Render debug images showing each effect independently isolated and
        /// effect chain from top-down/bottom-up.
        #[arg(long)]
        debug_effects: bool,

        /// Path to a TOML config file for disabling effects.
        #[arg(long)]
        config: Option<PathBuf>,
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
            auto_pair,
            debug_layout,
            debug_effects,
            config,
        } => {
            let config: Config = config
                .as_ref()
                .map(|p| Config::from_file(p.as_ref()))
                .transpose()?
                .unwrap_or_default();
            let disabled = config.disabled_effects_slice();

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

            let mut cache = am_renderer::render::compositor::ImageCache::new();
            if auto_pair {
                let mappings = build_virtual_mappings(&project, &assets)?;
                println!("Auto-pairing virtually ({} mapping(s) created):", mappings.len());
                for (uri, path) in &mappings {
                    let filename = uri.rsplit("///").next().unwrap_or(uri).trim_start_matches('/');
                    println!("  {} -> {}", filename, path.file_name().unwrap_or_default().to_string_lossy());
                }
                cache.set_virtual_mappings(mappings);
            }

            if let Some(f) = frame {
                let time_secs = f as f32 / project.fps;
                let resolved = am_renderer::eval::timeline::evaluate(&project, time_secs);
                if dump_graph {
                    print_render_graph(&resolved, f, time_secs);
                }
                let img = am_renderer::render::compositor::render_scene(&resolved, &mut cache, &assets, debug_layout, disabled)?;

                let out_dir = if fmt == Format::Png {
                    output
                } else {
                    output.parent().unwrap_or(&output).to_path_buf()
                };

                if debug_effects {
                    render_effects_debug(&resolved, &mut cache, &assets, f, &out_dir, disabled)?;
                    println!("Debug effects images saved to {}/debug_effects_frame{:06}", out_dir.display(), f);
                }

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
                        am_renderer::export::png::export_sequence(&project, &assets, &output, None, None, &mut cache, debug_layout, disabled)?;
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
                        am_renderer::export::png::export_sequence(&project, &assets, &temp_dir, None, None, &mut cache, debug_layout, disabled)?;

                        let video_w = if debug_layout { project.width * 2 } else { project.width };
                        let video_h = if debug_layout { project.height * 2 } else { project.height };
                        am_renderer::export::video::export_mp4(
                            &temp_dir,
                            &output,
                            project.fps as u32,
                            video_w,
                            video_h,
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

