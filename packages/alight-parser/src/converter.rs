//! XML-to-domain-model converter for the Alight Motion renderer.

use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use graph_resolver::model::effect::*;
use graph_resolver::model::*;
use crate::types::*;
use graph_resolver::utils::parse_hex_color;

fn scale_animated_vec3(mut anim: Animated<[f32; 3]>, scale: f32) -> Animated<[f32; 3]> {
    if (scale - 1.0).abs() < 0.0001 {
        return anim;
    }
    match &mut anim {
        Animated::Static(v) => {
            v[0] *= scale;
            v[1] *= scale;
            v[2] *= scale;
        }
        Animated::Keyframed(kfs) => {
            for kf in kfs {
                kf.value[0] *= scale;
                kf.value[1] *= scale;
                kf.value[2] *= scale;
            }
        }
    }
    anim
}

pub fn convert_project(xml: &XmlScene, proxy_scale: Option<f32>) -> Result<Project> {
    let scale = proxy_scale.unwrap_or(1.0);
    let width = (xml.width.parse::<f32>().context("invalid scene width")? * scale).round() as u32;
    let height = (xml.height.parse::<f32>().context("invalid scene height")? * scale).round() as u32;
    let export_width = xml
        .export_width
        .as_deref()
        .and_then(|w| w.parse::<f32>().ok())
        .map(|w| (w * scale).round() as u32)
        .unwrap_or(width);
    let export_height = xml
        .export_height
        .as_deref()
        .and_then(|h| h.parse::<f32>().ok())
        .map(|h| (h * scale).round() as u32)
        .unwrap_or(height);

    let bg_color = xml
        .bgcolor
        .as_deref()
        .map(parse_hex_color)
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);

    // Coordinate scale: XML uses a logical coordinate system where
    // 1 logical unit = 2 pixels at the render canvas resolution.
    // See AGENTS.md for full documentation.
    let coord_scale = 2.0 * scale;

    let total_time = xml.total_time.parse().context("invalid totalTime")?;
    let fps = xml.fps.parse().context("invalid fps")?;

    let media = xml.media().into_iter().map(convert_media).collect();
    let audio_tracks = xml.audio().into_iter().map(convert_audio).collect();

    let mut layers = Vec::new();
    for shape in xml.shapes() {
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
                    .map(|l| scale_animated_vec3(convert_animated_vec3(l, [0.0, 0.0, 0.0]), scale))
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
            Some("add") | Some("linear-dodge") => BlendMode::Add,
            _ => BlendMode::Normal,
        };

        let raw_size = shape
            .properties
            .iter()
            .find(|p| p.name == "size")
            .and_then(|p| p.value.as_deref())
            .map(|v| parse_vec2(v, [100.0, 100.0]))
            .unwrap_or([100.0, 100.0]);
        // Alight Motion XML size values are in logical points (half-canvas coords).
        // Multiply by coord_scale to convert to pixel dimensions.
        let size = [raw_size[0] * coord_scale, raw_size[1] * coord_scale];

        let effects = shape
            .effects
            .iter()
            .map(|e| convert_effect(e, coord_scale, scale))
            .collect();

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
            media_fill_mode: shape.media_fill_mode.clone(),
            effects,
            size,
            s: shape.s.clone(),
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
        in_time: xml
            .in_time
            .as_deref()
            .and_then(|t| t.parse().ok())
            .unwrap_or(0.0),
        out_time: xml.out_time.as_deref().and_then(|t| t.parse().ok()),
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
    let start_idx = if parts.first() == Some(&"local") {
        1
    } else {
        0
    };
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


fn get_prop_vec3(properties: &[XmlProperty], name: &str, default_val: [f32; 3]) -> [f32; 3] {
    properties
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.value.as_deref())
        .map(|v| parse_vec3(v, default_val))
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

fn convert_effect(xml: &XmlEffect, coord_scale: f32, scale: f32) -> Effect {
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
        "com.alightcreative.effects.fade" => {
            let in_val = get_prop_float(props, "inTime", get_prop_float(props, "in_time", 0.2));
            let out_val = get_prop_float(props, "outTime", get_prop_float(props, "out_time", 0.2));
            EffectType::Fade(FadeParams {
                in_time: in_val * 1000.0,
                out_time: out_val * 1000.0,
            })
        }
        "com.alightcreative.effects.tile" => EffectType::Tile(TileParams {
            mirror: get_prop_bool(props, "mirror", false),
            scale: get_prop_animated_float(props, "scale", 1.0),
            phase: get_prop_animated_float(props, "phase", 0.0),
            vert_offset: get_prop_bool(props, "vertoffs", false),
            angle: get_prop_animated_float(props, "angle", 0.0),
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
        "com.alightcreative.effects.saturationvibrance" | "com.alightcreative.effects.satvib" => {
            EffectType::SaturationVibrance(SaturationVibranceParams {
                saturation: get_prop_float(props, "saturation", 0.0),
                vibrance: get_prop_float(props, "vibrance", 0.0),
            })
        }
        "com.alightcreative.effects.colortint" => EffectType::ColorTint(ColorTintParams {
            tint: get_prop_color3(props, "tint", [1.0, 1.0, 1.0]),
        }),
        "com.alightcreative.effects.colorize" => EffectType::Colorize(ColorizeParams {
            tint: get_prop_vec3(props, "tint", [0.0, 0.0, 0.0]),
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
            overlaycolor: get_prop_color4(props, "overlaycolor", [0.0, 0.0, 0.0, 1.0]),
            punchout: get_prop_bool(props, "punchout", false),
        }),
        "com.alightcreative.effects.sharpen" => EffectType::Sharpen(SharpenParams {
            radius: get_prop_float(props, "radius", 1.0) * coord_scale,
            strength: get_prop_float(props, "strength", 0.5),
        }),
        "com.alightcreative.effects.gaussianblur" => EffectType::GaussianBlur(GaussianBlurParams {
            radius: get_prop_float(
                props,
                "radius",
                get_prop_float(props, "strength", 0.05) * 100.0,
            ) * coord_scale,
        }),
        "com.alightcreative.effects.lensblur" => EffectType::LensBlur(LensBlurParams {
            radius: get_prop_float(props, "radius", 5.0) * coord_scale,
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
        "com.alightcreative.effects.lift" => EffectType::Lift(LiftParams {
            fill: get_prop_float(props, "fill", 0.0),
        }),
        "com.alightcreative.effects.lumakey" => EffectType::LumaKey(LumaKeyParams {
            low_threshold: get_prop_animated_float(props, "low_threshold", 0.0),
            high_threshold: get_prop_animated_float(props, "high_threshold", 1.0),
        }),
        "com.alightcreative.effects.offset" => EffectType::Offset(OffsetParams {
            offset: {
                let raw_offset = get_prop_vec2(props, "offset", [0.0, 0.0]);
                [raw_offset[0] * scale, raw_offset[1] * scale]
            },
            feather: get_prop_float(props, "feather", 0.0),
            mask: get_prop_bool(props, "mask", false),
        }),
        "com.alightcreative.effects.findedges" => EffectType::FindEdges(FindEdgesParams {
            smoothing: get_prop_float(props, "smoothing", 1.0),
            threshold: get_prop_float(props, "threshold", 1.0),
            invert: get_prop_bool(props, "invert", true),
        }),
        "com.alightcreative.effects.stretchsegment" => {
            EffectType::StretchSegment(StretchSegmentParams {
                angle: get_prop_float(props, "angle", 0.0),
                stretch: get_prop_float(props, "stretch", 0.0) * scale,
                offset: get_prop_float(props, "offset", 0.0) * scale,
                smooth: get_prop_float(props, "smooth", 0.0),
            })
        }
        "com.alightcreative.effects.swirl4"
        | "com.alightcreative.effects.swirl3"
        | "com.alightcreative.effects.swirl2"
        | "com.alightcreative.effects.swirl" => {
            let exponent = match xml.id.as_str() {
                "com.alightcreative.effects.swirl2" => 2,
                "com.alightcreative.effects.swirl3" => 3,
                "com.alightcreative.effects.swirl4" => 4,
                _ => 1,
            };
            EffectType::Swirl(SwirlParams {
                strength: get_prop_float(props, "strength", 0.0),
                radius: get_prop_float(props, "radius", 0.5),
                exponent,
            })
        }
        "com.alightcreative.effects.spin" => EffectType::Spin(SpinParams {
            rpm: get_prop_animated_float(props, "rpm", 0.0),
        }),
        "com.alightcreative.effects.wipe2" | "com.alightcreative.effects.wipe" => {
            EffectType::Wipe(WipeParams {
                start: get_prop_animated_float(props, "start", 0.0),
                end: get_prop_animated_float(props, "end", 1.0),
                angle: get_prop_animated_float(props, "angle", 0.0),
                feather: get_prop_float(props, "feather", 0.0),
            })
        }
        other => EffectType::Unknown(other.to_string()),
    };

    Effect {
        effect_type,
        locally_applied,
    }
}

pub fn build_virtual_mappings(
    project: &Project,
    assets_dir: &Path,
) -> Result<HashMap<String, PathBuf>> {
    let mut mappings = HashMap::new();

    // 1. Gather all unique image and audio URIs required
    let mut required_image_uris = HashSet::new();
    for layer in &project.layers {
        if layer.fill_type == FillType::Media {
            if let Some(ref uri) = layer.fill_image {
                required_image_uris.insert(uri.clone());
            }
        }
    }
    for m in &project.media {
        let is_audio = m
            .mime_type
            .as_deref()
            .map(|t| t.starts_with("audio/"))
            .unwrap_or(false)
            || m.uri.ends_with(".mp3")
            || m.uri.ends_with(".wav")
            || m.uri.ends_with(".m4a");
        if !is_audio {
            required_image_uris.insert(m.uri.clone());
        }
    }

    let mut required_audio_uris = HashSet::new();
    for track in &project.audio_tracks {
        if let Some(ref uri) = track.src {
            required_audio_uris.insert(uri.clone());
        }
    }

    // 2. Scan assets directory for available physical files
    let mut available_images = Vec::new();
    let mut available_audio = Vec::new();

    if assets_dir.exists() {
        for entry in std::fs::read_dir(assets_dir)?.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if matches!(
                        ext_lower.as_str(),
                        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
                    ) {
                        available_images.push(path);
                    } else if matches!(
                        ext_lower.as_str(),
                        "mp3" | "wav" | "m4a" | "ogg" | "aac" | "flac" | "mp4"
                    ) {
                        available_audio.push(path);
                    }
                }
            }
        }
    }

    // 3. Pair images
    if !required_image_uris.is_empty() {
        if available_images.is_empty() {
            anyhow::bail!(
                "No source images found in assets directory '{}' to perform auto-pairing.",
                assets_dir.display()
            );
        }
        available_images.sort();
        let mut sorted_uris: Vec<String> = required_image_uris.into_iter().collect();
        sorted_uris.sort();
        for (idx, uri) in sorted_uris.into_iter().enumerate() {
            let physical_path = &available_images[idx % available_images.len()];
            mappings.insert(uri, physical_path.clone());
        }
    }

    // 4. Pair audio
    if !required_audio_uris.is_empty() {
        if available_audio.is_empty() {
            anyhow::bail!(
                "No source audio files found in assets directory '{}' to perform auto-pairing.",
                assets_dir.display()
            );
        }
        available_audio.sort();
        let mut sorted_audio_uris: Vec<String> = required_audio_uris.into_iter().collect();
        sorted_audio_uris.sort();
        for (idx, uri) in sorted_audio_uris.into_iter().enumerate() {
            let physical_path = &available_audio[idx % available_audio.len()];
            mappings.insert(uri, physical_path.clone());
        }
    }

    Ok(mappings)
}
