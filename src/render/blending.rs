//! Blend mode implementations for pixel compositing.
//!
//! All blend modes operate in linear RGB space and handle alpha correctly
//! using standard Porter-Duff "over" compositing with per-mode color blending.

use crate::model::BlendMode;
use image::Rgba;

/// Blend a source pixel onto a destination pixel using the specified blend mode and opacity.
///
/// The source pixel's alpha is multiplied by `opacity` before blending.
/// Uses Porter-Duff "over" compositing with the blend mode applied to color channels.
///
/// # Arguments
/// * `dst` - Destination pixel (background)
/// * `src` - Source pixel (foreground layer)
/// * `mode` - Blend mode to use
/// * `opacity` - Layer opacity multiplier (0.0 - 1.0)
///
/// # Returns
/// The composited pixel.
pub fn blend_pixel(dst: Rgba<u8>, src: Rgba<u8>, mode: BlendMode, opacity: f32) -> Rgba<u8> {
    // Convert to float [0, 1]
    let sr = src[0] as f32 / 255.0;
    let sg = src[1] as f32 / 255.0;
    let sb = src[2] as f32 / 255.0;
    let sa = (src[3] as f32 / 255.0) * opacity;

    let dr = dst[0] as f32 / 255.0;
    let dg = dst[1] as f32 / 255.0;
    let db = dst[2] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;

    // Early exit if source is fully transparent
    if sa < 1.0 / 255.0 {
        return dst;
    }

    // Apply blend mode to color channels
    let (br, bg, bb) = match mode {
        BlendMode::Normal => (sr, sg, sb),
        BlendMode::Multiply => (sr * dr, sg * dg, sb * db),
        BlendMode::Screen => (
            1.0 - (1.0 - sr) * (1.0 - dr),
            1.0 - (1.0 - sg) * (1.0 - dg),
            1.0 - (1.0 - sb) * (1.0 - db),
        ),
        BlendMode::Overlay => (
            overlay_channel(dr, sr),
            overlay_channel(dg, sg),
            overlay_channel(db, sb),
        ),
        BlendMode::Darken => (sr.min(dr), sg.min(dg), sb.min(db)),
        BlendMode::Lighten => (sr.max(dr), sg.max(dg), sb.max(db)),
        BlendMode::Subtract => (
            (dr - sr).max(0.0),
            (dg - sg).max(0.0),
            (db - sb).max(0.0),
        ),
        BlendMode::Add => (
            (dr + sr).min(1.0),
            (dg + sg).min(1.0),
            (db + sb).min(1.0),
        ),
    };

    // Porter-Duff "over" compositing
    let out_a = sa + da * (1.0 - sa);
    if out_a < 1.0 / 255.0 {
        return Rgba([0, 0, 0, 0]);
    }

    let out_r = (br * sa + dr * da * (1.0 - sa)) / out_a;
    let out_g = (bg * sa + dg * da * (1.0 - sa)) / out_a;
    let out_b = (bb * sa + db * da * (1.0 - sa)) / out_a;

    Rgba([
        (out_r * 255.0).round().clamp(0.0, 255.0) as u8,
        (out_g * 255.0).round().clamp(0.0, 255.0) as u8,
        (out_b * 255.0).round().clamp(0.0, 255.0) as u8,
        (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
    ])
}

/// Overlay blend for a single channel.
///
/// If base < 0.5: 2 * base * blend
/// Else: 1 - 2 * (1 - base) * (1 - blend)
fn overlay_channel(base: f32, blend: f32) -> f32 {
    if base < 0.5 {
        2.0 * base * blend
    } else {
        1.0 - 2.0 * (1.0 - base) * (1.0 - blend)
    }
}
