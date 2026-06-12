use image::RgbaImage;
use rayon::prelude::*;

pub fn apply_tile(
    img: RgbaImage,
    scale: f32,
    phase: f32,
    vert_offset: bool,
    mirror: bool,
    angle: f32,
) -> RgbaImage {
    let w = img.width();
    let h = img.height();

    if scale <= 0.0 {
        return img;
    }

    let mut result = RgbaImage::new(w, h);
    let src_w = w as f32;
    let src_h = h as f32;

    let crop = 0.01f32;
    let cx = src_w * crop;
    let cy = src_h * crop;
    let eff_w = (src_w - 2.0 * cx).max(1.0);
    let eff_h = (src_h - 2.0 * cy).max(1.0);

    let angle_rad = angle.to_radians();
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();
    let has_rotation = angle.abs() > 0.001;

    let w_u = w as usize;
    let stride = w_u * 4;
    let result_raw = result.as_mut();

    result_raw
        .par_chunks_mut(stride)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w {
                let tx = (x as f32 / src_w) * scale;
                let ty = (y as f32 / src_h) * scale;

                let col = tx.floor() as i32;
                let row_t = ty.floor() as i32;
                let mut fx = tx.fract();
                let mut fy = ty.fract();

                if fx < 0.0 {
                    fx += 1.0;
                }
                if fy < 0.0 {
                    fy += 1.0;
                }

                if phase != 0.0 {
                    if vert_offset {
                        if col.rem_euclid(2) == 1 {
                            fy = (fy + phase).rem_euclid(1.0);
                        }
                    } else if row_t.rem_euclid(2) == 1 {
                        fx = (fx + phase).rem_euclid(1.0);
                    }
                }

                if mirror {
                    if col.rem_euclid(2) == 1 {
                        fx = 1.0 - fx;
                    }
                    if row_t.rem_euclid(2) == 1 {
                        fy = 1.0 - fy;
                    }
                }

                if has_rotation {
                    let rfx = fx - 0.5;
                    let rfy = fy - 0.5;
                    fx = (cos_a * rfx - sin_a * rfy + 0.5).rem_euclid(1.0);
                    fy = (sin_a * rfx + cos_a * rfy + 0.5).rem_euclid(1.0);
                }

                let sx = ((cx + fx * eff_w).round() as u32).min(w - 1);
                let sy = ((cy + fy * eff_h).round() as u32).min(h - 1);

                let src_px = img.get_pixel(sx, sy);
                let dst_idx = x as usize * 4;
                row[dst_idx] = src_px[0];
                row[dst_idx + 1] = src_px[1];
                row[dst_idx + 2] = src_px[2];
                row[dst_idx + 3] = src_px[3];
            }
        });

    result
}