//! UV effect implementations — tiling and mirroring.
//!
//! These effects modify UV (texture) coordinates during sampling to
//! create repeating patterns.

use image::{RgbaImage, Rgba};

/// Apply tile/mirror effect to an image.
///
/// Creates a tiled version of the source image. Implements the full Alight
/// Motion Tiles effect: crop, scale, phase stagger (brick pattern),
/// mirror-flip alternating tiles, and per-tile rotation.
///
/// # Algorithm
///
/// For each output pixel:
/// 1. **Crop** 1% from each edge to prevent seam bleeding.
/// 2. **Tile-space**: divide output into `scale×scale` tiles using
///    fractional decomposition.
/// 3. **Offset**: shift odd rows by `phase` (or odd columns if
///    `vert_offset` is true) — creates brick/pinwheel patterns.
/// 4. **Mirror**: horizontally flip odd columns, vertically flip odd rows.
/// 5. **Rotate**: rotate local UV around each tile's centre by `angle`.
/// 6. **Sample**: map the local UV back into the cropped source region.
///
/// # Arguments
/// * `img` - Source image
/// * `scale` - Number of tiles in each direction (1.0 = no tiling)
/// * `phase` - Stagger offset for alternating rows/columns as fraction of tile
/// * `vert_offset` - If true, stagger columns instead of rows
/// * `mirror` - Whether to mirror alternating tiles
/// * `angle` - Per-tile rotation in degrees
///
/// # Returns
/// New image with tiling applied (same dimensions as source).
pub fn apply_tile(
    img: &RgbaImage,
    scale: f32,
    phase: f32,
    vert_offset: bool,
    mirror: bool,
    angle: f32,
) -> RgbaImage {
    let w = img.width();
    let h = img.height();

    if scale <= 0.0 {
        return img.clone();
    }

    let mut result = RgbaImage::new(w, h);
    let src_w = w as f32;
    let src_h = h as f32;

    // Step 1: Crop 1% from each edge to prevent seam bleeding
    let crop = 0.01f32;
    let cx = src_w * crop;
    let cy = src_h * crop;
    let eff_w = (src_w - 2.0 * cx).max(1.0);
    let eff_h = (src_h - 2.0 * cy).max(1.0);

    // Precompute per-tile rotation
    let angle_rad = angle.to_radians();
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();
    let has_rotation = angle.abs() > 0.001;

    for y in 0..h {
        for x in 0..w {
            // Step 2: Tile-space coordinates
            let tx = (x as f32 / src_w) * scale;
            let ty = (y as f32 / src_h) * scale;

            let col = tx.floor() as i32;
            let row = ty.floor() as i32;
            let mut fx = tx.fract();
            let mut fy = ty.fract();

            if fx < 0.0 {
                fx += 1.0;
            }
            if fy < 0.0 {
                fy += 1.0;
            }

            // Step 3: Offset (brick stagger) — shift alternate rows/columns
            if phase != 0.0 {
                if vert_offset {
                    if col.rem_euclid(2) == 1 {
                        fy = (fy + phase).rem_euclid(1.0);
                    }
                } else if row.rem_euclid(2) == 1 {
                    fx = (fx + phase).rem_euclid(1.0);
                }
            }

            // Step 4: Mirror alternating tiles
            if mirror {
                if col.rem_euclid(2) == 1 {
                    fx = 1.0 - fx;
                }
                if row.rem_euclid(2) == 1 {
                    fy = 1.0 - fy;
                }
            }

            // Step 5: Per-tile rotation around tile centre
            if has_rotation {
                let rfx = fx - 0.5;
                let rfy = fy - 0.5;
                fx = (cos_a * rfx - sin_a * rfy + 0.5).rem_euclid(1.0);
                fy = (sin_a * rfx + cos_a * rfy + 0.5).rem_euclid(1.0);
            }

            // Step 6: Map to source pixel with crop offset
            let sx = ((cx + fx * eff_w).round() as u32).min(w - 1);
            let sy = ((cy + fy * eff_h).round() as u32).min(h - 1);

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

    let w_i32 = w as i32;
    let h_i32 = h as i32;
    let dx_wrapped = ((dx.round() as i32) % w_i32 + w_i32) % w_i32;
    let dy_wrapped = ((dy.round() as i32) % h_i32 + h_i32) % h_i32;
    let dx_w = dx_wrapped as u32;
    let dy_h = dy_wrapped as u32;

    let src_raw = img.as_raw();
    let dst_raw = result.as_mut();
    let w4 = w as usize * 4;

    for y in 0..h {
        let sy = if y >= dy_h { y - dy_h } else { y + (h - dy_h) };
        let sy_off = sy as usize * w4;
        let dy_off = y as usize * w4;

        for x in 0..w {
            let sx = if x >= dx_w { x - dx_w } else { x + (w - dx_w) };
            let src_idx = sy_off + sx as usize * 4;
            let dst_idx = dy_off + x as usize * 4;
            dst_raw[dst_idx] = src_raw[src_idx];
            dst_raw[dst_idx + 1] = src_raw[src_idx + 1];
            dst_raw[dst_idx + 2] = src_raw[src_idx + 2];
            dst_raw[dst_idx + 3] = src_raw[src_idx + 3];
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

    let angle_rad = angle.to_radians();
    let cx = w as f32 / 2.0 + offset * angle_rad.sin();
    let cy = h as f32 / 2.0 + offset * angle_rad.cos();
    let half_stretch = stretch / 2.0;
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();
    let w_m1 = w as f32 - 1.0;
    let h_m1 = h as f32 - 1.0;

    let src_raw = img.as_raw();
    let dst_raw = result.as_mut();

    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let proj = dx * sin_a - dy * cos_a;

            let (sx, sy) = if proj > half_stretch {
                let src_x = x as f32 - sin_a * half_stretch;
                let src_y = y as f32 + cos_a * half_stretch;
                (src_x.round().clamp(0.0, w_m1) as u32,
                 src_y.round().clamp(0.0, h_m1) as u32)
            } else if proj < -half_stretch {
                let src_x = x as f32 + sin_a * half_stretch;
                let src_y = y as f32 - cos_a * half_stretch;
                (src_x.round().clamp(0.0, w_m1) as u32,
                 src_y.round().clamp(0.0, h_m1) as u32)
            } else {
                let t = proj.clamp(-half_stretch, half_stretch);
                let src_x = (cx + t * sin_a).round().clamp(0.0, w_m1) as u32;
                let src_y = (cy - t * cos_a).round().clamp(0.0, h_m1) as u32;
                (src_x, src_y)
            };

            let src_idx = (sy * w + sx) as usize * 4;
            let dst_idx = (y * w + x) as usize * 4;
            dst_raw[dst_idx] = src_raw[src_idx];
            dst_raw[dst_idx + 1] = src_raw[src_idx + 1];
            dst_raw[dst_idx + 2] = src_raw[src_idx + 2];
            dst_raw[dst_idx + 3] = src_raw[src_idx + 3];
        }
    }

    result
}

