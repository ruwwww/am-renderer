use anyhow::Result;
use image::RgbaImage;
use log::warn;

use graph_resolver::model::{Effect, EffectType, ResolvedLayer};

pub mod blink;
pub mod brightness_contrast;
pub mod color_tint;
pub mod colorize;
pub mod exposure;
pub mod find_edges;
pub mod gaussian_blur;
pub mod gradient_overlay;
pub mod highlight_shadow;
pub mod hsl;
pub mod lens_blur;
pub mod lift;
pub mod luma_key;
pub mod motion_blur;
pub mod offset;
pub mod sharpen;
pub mod stretch_segment;
pub mod swirl;
pub mod tile;
pub mod transform;
pub mod vignette;
pub mod wipe;

pub use transform::apply_transform_effects;

pub fn apply_pixel_effects(
    effects: &[Effect],
    img: RgbaImage,
    layer: &ResolvedLayer,
    disabled_effects: &[String],
) -> Result<RgbaImage> {
    let mut img = img;

    for effect in effects {
        if disabled_effects
            .iter()
            .any(|d| d == effect.effect_type.type_name())
        {
            continue;
        }
        img = match &effect.effect_type {
            EffectType::Exposure(params) => {
                let exp = params.exposure.evaluate(layer.normalized_t);
                let gamma = params.gamma.evaluate(layer.normalized_t);
                let offset = params.offset;
                if exp.abs() > 0.01 || (gamma - 1.0).abs() > 0.01 || offset.abs() > 0.01 {
                    exposure::apply_exposure(img, exp, gamma, offset)
                } else {
                    img
                }
            }
            EffectType::GaussianBlur(params) => {
                if params.radius > 0.5 {
                    gaussian_blur::gaussian_blur(&img, params.radius)
                } else {
                    img
                }
            }
            EffectType::Vignette(params) => {
                if params.strength.abs() > 0.001 {
                    vignette::apply_vignette(
                        img,
                        params.feather,
                        params.roundness,
                        params.scale,
                        params.strength,
                        params.tint,
                        params.overlaycolor,
                        params.punchout,
                    )
                } else {
                    img
                }
            }
            EffectType::BrightnessContrast(params) => {
                if params.brightness.abs() > 0.001 || params.contrast.abs() > 0.001 {
                    brightness_contrast::apply_brightness_contrast(
                        img,
                        params.brightness,
                        params.contrast,
                    )
                } else {
                    img
                }
            }
            EffectType::SaturationVibrance(params) => {
                if params.saturation.abs() > 0.001 {
                    hsl::apply_hsl(img, 0.0, params.saturation, 0.0)
                } else {
                    img
                }
            }
            EffectType::ColorTint(params) => {
                let color = [params.tint[0], params.tint[1], params.tint[2], 1.0];
                color_tint::apply_color_fill(img, color, 1.0)
            }
            EffectType::Colorize(params) => {
                colorize::apply_colorize(img, params.tint[0], params.tint[1])
            }
            EffectType::FindEdges(params) => {
                find_edges::find_edges(img, params.smoothing, params.threshold, params.invert)
            }
            EffectType::StretchSegment(params) => {
                if params.stretch.abs() > 0.5 {
                    stretch_segment::apply_stretch_segment(
                        img,
                        params.angle,
                        params.stretch,
                        params.offset,
                        params.smooth,
                    )
                } else {
                    img
                }
            }
            EffectType::Swirl(params) => {
                if effect.locally_applied && params.strength.abs() > 0.001 {
                    swirl::apply_swirl(img, params.strength, params.radius, params.exponent)
                } else {
                    img
                }
            }
            EffectType::Offset(params) => {
                if params.offset[0].abs() > 0.5 || params.offset[1].abs() > 0.5 {
                    offset::apply_offset(img, params.offset[0], params.offset[1])
                } else {
                    img
                }
            }
            EffectType::Tile(params) => {
                if effect.locally_applied {
                    let scale = params.scale.evaluate(layer.normalized_t);
                    let phase = params.phase.evaluate(layer.normalized_t);
                    let angle = params.angle.evaluate(layer.normalized_t);
                    if (scale - 1.0).abs() > 0.01 || angle.abs() > 0.001 || phase.abs() > 0.001 || params.mirror {
                        tile::apply_tile(img, scale, phase, params.vert_offset, params.mirror, angle)
                    } else {
                        img
                    }
                } else {
                    img
                }
            }
            EffectType::HighlightShadow(params) => {
                highlight_shadow::apply_highlight_shadow(img, params.highlights, params.shadows)
            }
            EffectType::GradientOverlay(params) => gradient_overlay::apply_gradient_overlay(
                img,
                params.alpha,
                params.color1,
                params.color2,
                params.offset,
                params.scale,
            ),
            EffectType::LumaKey(params) => {
                let low = params.low_threshold.evaluate(layer.normalized_t);
                let high = params.high_threshold.evaluate(layer.normalized_t);
                luma_key::apply_luma_key(img, low, high)
            }
            EffectType::LensBlur(params) => {
                lens_blur::apply_lens_blur(img, params.radius, params.strength)
            }
            EffectType::Sharpen(params) => {
                sharpen::apply_sharpen(img, params.radius, params.strength)
            }
            EffectType::Blink(params) => {
                let freq = params.freq.evaluate(layer.normalized_t);
                blink::apply_blink(img, freq, layer.time_secs)
            }
            EffectType::MotionBlur(params) => {
                let tune = params.tune.evaluate(layer.normalized_t);
                motion_blur::apply_motion_blur(img, tune, layer.time_secs, layer.location)
            }
            // Transform effects handled separately via apply_transform_effects / timeline
            EffectType::Oscillate(_)
            | EffectType::Swing(_)
            | EffectType::RandomDisplace(_)
            | EffectType::Spin(_) => img,
            // Lift handled separately in create_layer_source
            EffectType::Lift(_) => img,
            // Fade handled separately in resolve_layer
            EffectType::Fade(_) => img,
            EffectType::Wipe(params) => {
                if effect.locally_applied {
                    let start = params.start.evaluate(layer.normalized_t);
                    let end = params.end.evaluate(layer.normalized_t);
                    let angle = params.angle.evaluate(layer.normalized_t);
                    wipe::apply_wipe(img, start, end, angle, params.feather)
                } else {
                    img
                }
            }
            EffectType::Unknown(id) => {
                warn!("Unknown effect type skipped: {}", id);
                img
            }
        };
    }

    Ok(img)
}
