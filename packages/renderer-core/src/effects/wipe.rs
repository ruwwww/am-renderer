use image::RgbaImage;
use rayon::prelude::*;

/// Apply the Wipe transition/mask effect to a pixel buffer.
///
/// Projects each coordinate along the angle to find its normalized position `u`
/// in `[0.0, 1.0]`. Pixels outside the `[start, end]` range (or vice-versa)
/// are masked out with optional feathering.
pub fn apply_wipe(
    img: RgbaImage,
    start: f32,
    end: f32,
    angle: f32,
    feather: f32,
) -> RgbaImage {
    let w = img.width();
    let h = img.height();
    if w == 0 || h == 0 {
        return img;
    }

    let mut result = img.clone();
    let angle_rad = angle.to_radians();
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    // Determine the min and max projections of the 4 corners of the image
    let p00 = 0.0f32;
    let p_w0 = w as f32 * cos_a;
    let p0_h = h as f32 * sin_a;
    let p_wh = w as f32 * cos_a + h as f32 * sin_a;

    let min_proj = p00.min(p_w0).min(p0_h).min(p_wh);
    let max_proj = p00.max(p_w0).max(p0_h).max(p_wh);
    let range = max_proj - min_proj;

    if range <= 0.001 {
        return result;
    }

    // Swap start and end if start > end
    let (s, e) = if start <= end { (start, end) } else { (end, start) };

    let feather_u = if feather > 0.0 { feather / range } else { 0.0 };

    let stride = w as usize * 4;
    let raw = result.as_mut();

    raw.par_chunks_mut(stride)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w {
                let proj = x as f32 * cos_a + y as f32 * sin_a;
                let u = (proj - min_proj) / range;

                let factor = if u < s {
                    if feather_u > 0.0 {
                        ((feather_u - (s - u)) / feather_u).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                } else if u > e {
                    if feather_u > 0.0 {
                        ((feather_u - (u - e)) / feather_u).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                } else {
                    1.0
                };

                if factor < 1.0 {
                    let idx = x as usize * 4;
                    row[idx + 3] = (row[idx + 3] as f32 * factor).round() as u8;
                }
            }
        });

    result
}
