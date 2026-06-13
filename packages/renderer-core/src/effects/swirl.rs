//! Swirl warp effect.
//!
//! Rotates pixels around the layer center, creating a vortex/whirlpool distortion
//! with radial falloff.

use image::RgbaImage;
use rayon::prelude::*;

/// Sample the image with bilinear interpolation.
/// `u`, `v` are in [0, 1] normalized UV space.
#[inline]
fn sample_bilinear(src: &[u8], w: usize, h: usize, u: f32, v: f32) -> [u8; 4] {
    // Map to texel space, offset by -0.5 so (0,0) = center of first texel
    let fx = u * w as f32 - 0.5;
    let fy = v * h as f32 - 0.5;

    let x0 = fx.floor() as isize;
    let y0 = fy.floor() as isize;
    let x1 = x0 + 1;
    let y1 = y0 + 1;

    // Fractional parts for weighting
    let tx = fx - fx.floor();
    let ty = fy - fy.floor();

    // Clamp to valid texel range
    let cx0 = x0.clamp(0, w as isize - 1) as usize;
    let cy0 = y0.clamp(0, h as isize - 1) as usize;
    let cx1 = x1.clamp(0, w as isize - 1) as usize;
    let cy1 = y1.clamp(0, h as isize - 1) as usize;

    let stride = w * 4;

    // Fetch 4 surrounding texels (RGBA)
    let fetch = |x: usize, y: usize| -> [f32; 4] {
        let idx = y * stride + x * 4;
        [
            src[idx] as f32,
            src[idx + 1] as f32,
            src[idx + 2] as f32,
            src[idx + 3] as f32,
        ]
    };

    let c00 = fetch(cx0, cy0);
    let c10 = fetch(cx1, cy0);
    let c01 = fetch(cx0, cy1);
    let c11 = fetch(cx1, cy1);

    // Bilinear blend: lerp horizontally, then vertically
    let mut out = [0u8; 4];
    for i in 0..4 {
        let top = c00[i] + (c10[i] - c00[i]) * tx;
        let bot = c01[i] + (c11[i] - c01[i]) * tx;
        out[i] = (top + (bot - top) * ty).round().clamp(0.0, 255.0) as u8;
    }
    out
}

pub fn apply_swirl(img: RgbaImage, strength: f32, radius: f32, exponent: i32) -> RgbaImage {
    let w = img.width() as usize;
    let h = img.height() as usize;
    let mut result = RgbaImage::new(w as u32, h as u32);

    let w_f = w as f32;
    let h_f = h as f32;
    let src_raw = img.as_raw();
    let stride = w * 4;
    let dst_raw = result.as_mut();

    let cx = 0.5f32;
    let cy = 0.5f32;
    let aspect = w_f / h_f;

    dst_raw
        .par_chunks_mut(stride)
        .enumerate()
        .for_each(|(y, row)| {
            // Texel-center-aligned UV
            let nv = (y as f32 + 0.5) / h_f;
            for x in 0..w {
                let nu = (x as f32 + 0.5) / w_f;

                let dx = nu - cx;
                let dy = nv - cy;

                // Aspect-corrected circular calculations
                let mut dx_c = dx * aspect;
                let mut dy_c = dy;
                let dist = (dx_c * dx_c + dy_c * dy_c).sqrt();

                if dist < radius {
                    let percent = (radius - dist) / radius;
                    let theta = percent.powi(exponent) * strength * 100.0;
                    let s = theta.sin();
                    let c = theta.cos();
                    let new_dx_c = dx_c * c - dy_c * s;
                    let new_dy_c = dx_c * s + dy_c * c;
                    dx_c = new_dx_c;
                    dy_c = new_dy_c;
                }

                let su = dx_c / aspect + cx;
                let sv = dy_c + cy;

                let rgba = sample_bilinear(src_raw, w, h, su, sv);
                let dst_idx = x * 4;
                row[dst_idx] = rgba[0];
                row[dst_idx + 1] = rgba[1];
                row[dst_idx + 2] = rgba[2];
                row[dst_idx + 3] = rgba[3];
            }
        });

    result
}
