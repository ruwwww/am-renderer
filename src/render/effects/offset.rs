use image::{Rgba, RgbaImage};
use rayon::prelude::*;

pub fn apply_offset(img: RgbaImage, dx: f32, dy: f32) -> RgbaImage {
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
