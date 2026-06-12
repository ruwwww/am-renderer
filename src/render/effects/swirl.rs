//! Swirl warp effect.
//!
//! Rotates pixels around the layer center, creating a vortex/whirlpool distortion
//! with radial falloff.

use image::RgbaImage;
use rayon::prelude::*;

pub fn apply_swirl(img: RgbaImage, strength: f32, radius: f32) -> RgbaImage {
    let w = img.width() as usize;
    let h = img.height() as usize;
    let mut result = RgbaImage::new(w as u32, h as u32);

    let w_f = w as f32;
    let h_f = h as f32;
    let src_raw = img.as_raw();
    let stride = w * 4;
    let dst_raw = result.as_mut();

    let center = [0.5f32, 0.5f32];

    dst_raw
        .par_chunks_mut(stride)
        .enumerate()
        .for_each(|(y, row)| {
            let ny = y as f32 / h_f;
            for x in 0..w {
                let nx = x as f32 / w_f;

                let mut dx = nx - center[0];
                let mut dy = ny - center[1];
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < radius {
                    let percent = (radius - dist) / radius;
                    let theta = percent * percent * strength * 8.0;
                    let s = theta.sin();
                    let c = theta.cos();
                    let new_dx = dx * c - dy * s;
                    let new_dy = dx * s + dy * c;
                    dx = new_dx;
                    dy = new_dy;
                }

                let sx = ((dx + center[0]) * w_f).round().clamp(0.0, w_f - 1.0) as usize;
                let sy = ((dy + center[1]) * h_f).round().clamp(0.0, h_f - 1.0) as usize;

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
