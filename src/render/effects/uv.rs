use image::{RgbaImage, Rgba};
use rayon::prelude::*;

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

pub fn sample_clamped(img: &RgbaImage, u: f32, v: f32) -> Rgba<u8> {
    let w = img.width() as f32;
    let h = img.height() as f32;

    let x = (u.clamp(0.0, 1.0) * (w - 1.0)).round() as u32;
    let y = (v.clamp(0.0, 1.0) * (h - 1.0)).round() as u32;

    *img.get_pixel(x.min(img.width() - 1), y.min(img.height() - 1))
}

pub fn apply_offset(img: &RgbaImage, dx: f32, dy: f32) -> RgbaImage {
    let w = img.width() as usize;
    let h = img.height() as usize;
    let mut result = RgbaImage::new(w as u32, h as u32);

    let w_i32 = w as i32;
    let h_i32 = h as i32;
    let dx_wrapped = ((dx.round() as i32) % w_i32 + w_i32) % w_i32;
    let dy_wrapped = ((dy.round() as i32) % h_i32 + h_i32) % h_i32;
    let dx_w = dx_wrapped as usize;
    let dy_h = dy_wrapped as usize;

    let src_raw = img.as_raw();
    let stride = w * 4;
    let dst_raw = result.as_mut();

    dst_raw
        .par_chunks_mut(stride)
        .enumerate()
        .for_each(|(y, row)| {
            let sy = if y >= dy_h { y - dy_h } else { y + (h - dy_h) };
            let sy_off = sy * stride;

            for x in 0..w {
                let sx = if x >= dx_w { x - dx_w } else { x + (w - dx_w) };
                let src_idx = sy_off + sx * 4;
                let dst_idx = x * 4;
                row[dst_idx] = src_raw[src_idx];
                row[dst_idx + 1] = src_raw[src_idx + 1];
                row[dst_idx + 2] = src_raw[src_idx + 2];
                row[dst_idx + 3] = src_raw[src_idx + 3];
            }
        });

    result
}

pub fn apply_stretch_segment(img: &RgbaImage, angle: f32, stretch: f32, offset: f32, _smooth: f32) -> RgbaImage {
    let w = img.width() as usize;
    let h = img.height() as usize;
    let mut result = RgbaImage::new(w as u32, h as u32);

    let angle_rad = angle.to_radians();
    let cx = w as f32 / 2.0 + offset * angle_rad.sin();
    let cy = h as f32 / 2.0 + offset * angle_rad.cos();
    let half_stretch = stretch / 2.0;
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();
    let w_m1 = w as f32 - 1.0;
    let h_m1 = h as f32 - 1.0;

    let src_raw = img.as_raw();
    let stride = w * 4;
    let dst_raw = result.as_mut();

    dst_raw
        .par_chunks_mut(stride)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let proj = dx * sin_a - dy * cos_a;

                let (sx, sy) = if proj > half_stretch {
                    let src_x = x as f32 - sin_a * half_stretch;
                    let src_y = y as f32 + cos_a * half_stretch;
                    (src_x.round().clamp(0.0, w_m1) as usize,
                     src_y.round().clamp(0.0, h_m1) as usize)
                } else if proj < -half_stretch {
                    let src_x = x as f32 + sin_a * half_stretch;
                    let src_y = y as f32 - cos_a * half_stretch;
                    (src_x.round().clamp(0.0, w_m1) as usize,
                     src_y.round().clamp(0.0, h_m1) as usize)
                } else {
                    let t = proj.clamp(-half_stretch, half_stretch);
                    let src_x = (cx + t * sin_a).round().clamp(0.0, w_m1) as usize;
                    let src_y = (cy - t * cos_a).round().clamp(0.0, h_m1) as usize;
                    (src_x, src_y)
                };

                let src_idx = sy * stride + sx * 4;
                let dst_idx = x * 4;
                row[dst_idx] = src_raw[src_idx];
                row[dst_idx + 1] = src_raw[src_idx + 1];
                row[dst_idx + 2] = src_raw[src_idx + 2];
                row[dst_idx + 3] = src_raw[src_idx + 3];
            }
        });

    result
}