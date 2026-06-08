//! UV effect implementations — tiling and mirroring.
//!
//! These effects modify UV (texture) coordinates during sampling to
//! create repeating patterns.

use image::{RgbaImage, Rgba};

/// Apply tile/mirror effect to an image.
///
/// Creates a tiled version of the source image. If `mirror` is true,
/// alternating tiles are flipped to create a seamless mirror pattern.
///
/// # Arguments
/// * `img` - Source image
/// * `tile_x` - Number of horizontal tiles (1.0 = no tiling)
/// * `tile_y` - Number of vertical tiles (1.0 = no tiling)
/// * `mirror` - Whether to mirror alternating tiles
///
/// # Returns
/// New image with tiling applied (same dimensions as source).
pub fn apply_tile(img: &RgbaImage, tile_x: f32, tile_y: f32, mirror: bool) -> RgbaImage {
    let w = img.width();
    let h = img.height();

    if tile_x <= 0.0 || tile_y <= 0.0 {
        return img.clone();
    }
    if (tile_x - 1.0).abs() < f32::EPSILON && (tile_y - 1.0).abs() < f32::EPSILON {
        return img.clone();
    }

    let mut result = RgbaImage::new(w, h);
    let src_w = w as f32;
    let src_h = h as f32;

    for y in 0..h {
        for x in 0..w {
            // Compute UV coordinates in tile space
            let u = (x as f32 / src_w) * tile_x;
            let v = (y as f32 / src_h) * tile_y;

            // Compute which tile we're in and local position
            let tile_ix = u.floor() as i32;
            let tile_iy = v.floor() as i32;
            let mut local_u = u.fract();
            let mut local_v = v.fract();

            // Handle negative fract
            if local_u < 0.0 {
                local_u += 1.0;
            }
            if local_v < 0.0 {
                local_v += 1.0;
            }

            // Mirror alternating tiles
            if mirror {
                if tile_ix % 2 != 0 {
                    local_u = 1.0 - local_u;
                }
                if tile_iy % 2 != 0 {
                    local_v = 1.0 - local_v;
                }
            }

            // Map back to source pixel coordinates
            let sx = (local_u * src_w).min(src_w - 1.0).max(0.0) as u32;
            let sy = (local_v * src_h).min(src_h - 1.0).max(0.0) as u32;

            result.put_pixel(x, y, *img.get_pixel(sx, sy));
        }
    }

    result
}

/// Sample a source image with UV wrapping (repeat mode).
///
/// Coordinates outside [0, 1] are wrapped using modular arithmetic.
///
/// # Arguments
/// * `img` - Source image
/// * `u` - Horizontal UV coordinate
/// * `v` - Vertical UV coordinate
///
/// # Returns
/// The sampled pixel color.
pub fn sample_wrapped(img: &RgbaImage, u: f32, v: f32) -> Rgba<u8> {
    let w = img.width() as f32;
    let h = img.height() as f32;

    let mut u = u % 1.0;
    let mut v = v % 1.0;
    if u < 0.0 {
        u += 1.0;
    }
    if v < 0.0 {
        v += 1.0;
    }

    let x = (u * w).min(w - 1.0).max(0.0) as u32;
    let y = (v * h).min(h - 1.0).max(0.0) as u32;

    *img.get_pixel(x, y)
}

/// Sample a source image with UV clamping.
///
/// Coordinates outside [0, 1] are clamped to the edge.
///
/// # Arguments
/// * `img` - Source image
/// * `u` - Horizontal UV coordinate
/// * `v` - Vertical UV coordinate
///
/// # Returns
/// The sampled pixel color.
pub fn sample_clamped(img: &RgbaImage, u: f32, v: f32) -> Rgba<u8> {
    let w = img.width() as f32;
    let h = img.height() as f32;

    let x = (u.clamp(0.0, 1.0) * (w - 1.0)).round() as u32;
    let y = (v.clamp(0.0, 1.0) * (h - 1.0)).round() as u32;

    *img.get_pixel(x.min(img.width() - 1), y.min(img.height() - 1))
}

/// Apply offset (scroll/shift) effect, wrapping content at bounding box edges.
///
/// This matches Alight Motion's Offset effect: the image content is shifted
/// by [dx, dy] pixels and wrapped around (tiled) at the boundaries.
///
/// # Arguments
/// * `img` - Source image
/// * `dx` - Horizontal offset in pixels (positive = shift right)
/// * `dy` - Vertical offset in pixels (positive = shift down)
///
/// # Notes (unofficial RE from XML)
/// - Parameter key `offset` is a vec2 in pixels.
/// - `feather` and `mask` params are parsed but not applied here (edge-fade
///   and masking are complex; they require canvas context not available here).
pub fn apply_offset(img: &RgbaImage, dx: f32, dy: f32) -> RgbaImage {
    let w = img.width();
    let h = img.height();
    let mut result = RgbaImage::new(w, h);

    let dx_i = dx.round() as i32;
    let dy_i = dy.round() as i32;

    for y in 0..h {
        for x in 0..w {
            // Wrap source coords
            let sx = ((x as i32 - dx_i).rem_euclid(w as i32)) as u32;
            let sy = ((y as i32 - dy_i).rem_euclid(h as i32)) as u32;
            result.put_pixel(x, y, *img.get_pixel(sx, sy));
        }
    }

    result
}

/// Apply stretch segment effect.
///
/// Slices the image along `angle` at its center, then moves the halves apart
/// by `stretch` pixels, filling the gap by repeating the boundary pixels.
///
/// # Arguments
/// * `img` - Source image
/// * `angle` - Cut angle in degrees (0 = horizontal split)
/// * `stretch` - Distance (pixels) to pull the two halves apart
/// * `offset` - Shift of the slice position along its perpendicular axis
/// * `smooth` - Feathering at the gap edges (currently approximated)
///
/// # Notes (unofficial RE from XML)
/// - `stretch=749` in the CC layer produces a full-canvas stretch/smear.
/// - The angle `0°` means the cut is horizontal (split top/bottom).
/// - The stretching repeats boundary pixels to fill the gap.
pub fn apply_stretch_segment(img: &RgbaImage, angle: f32, stretch: f32, offset: f32, _smooth: f32) -> RgbaImage {
    let w = img.width();
    let h = img.height();
    let mut result = RgbaImage::new(w, h);

    let cx = w as f32 / 2.0 + offset * angle.to_radians().sin();
    let cy = h as f32 / 2.0 + offset * angle.to_radians().cos();
    let half_stretch = stretch / 2.0;
    let cos_a = angle.to_radians().cos();
    let sin_a = angle.to_radians().sin();

    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            // Projection along the stretch direction (perpendicular to cut)
            let proj = dx * sin_a - dy * cos_a;

            let (sx, sy) = if proj > half_stretch {
                // Upper half: shift back
                let src_x = x as f32 - sin_a * half_stretch;
                let src_y = y as f32 + cos_a * half_stretch;
                (src_x.round().clamp(0.0, w as f32 - 1.0) as u32,
                 src_y.round().clamp(0.0, h as f32 - 1.0) as u32)
            } else if proj < -half_stretch {
                // Lower half: shift forward
                let src_x = x as f32 + sin_a * half_stretch;
                let src_y = y as f32 - cos_a * half_stretch;
                (src_x.round().clamp(0.0, w as f32 - 1.0) as u32,
                 src_y.round().clamp(0.0, h as f32 - 1.0) as u32)
            } else {
                // Gap: sample from nearest boundary pixel
                let t = proj.clamp(-half_stretch, half_stretch);
                let src_x = (cx + t * sin_a).round().clamp(0.0, w as f32 - 1.0) as u32;
                let src_y = (cy - t * cos_a).round().clamp(0.0, h as f32 - 1.0) as u32;
                (src_x, src_y)
            };

            result.put_pixel(x, y, *img.get_pixel(sx, sy));
        }
    }

    result
}

