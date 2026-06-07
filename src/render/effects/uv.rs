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
