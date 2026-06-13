use image::RgbaImage;
use rayon::prelude::*;

pub fn find_edges(img: RgbaImage, _smoothing: f32, threshold: f32, invert: bool) -> RgbaImage {
    let mut img = img;
    let w = img.width() as usize;
    let h = img.height() as usize;

    if w < 3 || h < 3 {
        return img;
    }

    let mut r_chan = vec![0.0f32; w * h];
    let mut g_chan = vec![0.0f32; w * h];
    let mut b_chan = vec![0.0f32; w * h];

    let w_u = w;
    let stride = w_u * 4;
    let raw_src = img.as_raw();

    r_chan
        .par_chunks_mut(w_u)
        .zip(g_chan.par_chunks_mut(w_u))
        .zip(b_chan.par_chunks_mut(w_u))
        .enumerate()
        .for_each(|(y, ((r_row, g_row), b_row))| {
            let offset = y * stride;
            for x in 0..w_u {
                r_row[x] = raw_src[offset + x * 4] as f32;
                g_row[x] = raw_src[offset + x * 4 + 1] as f32;
                b_row[x] = raw_src[offset + x * 4 + 2] as f32;
            }
        });

    let threshold_10 = threshold * 10.0;
    let raw = img.as_mut();

    raw.par_chunks_mut(stride).enumerate().for_each(|(y, row)| {
        if y == 0 || y == h - 1 {
            return;
        }
        for x in 1..(w - 1) {
            let idx = y * w + x;

            let gx_r = -r_chan[idx - w - 1] + r_chan[idx - w + 1] - 2.0 * r_chan[idx - 1]
                + 2.0 * r_chan[idx + 1]
                - r_chan[idx + w - 1]
                + r_chan[idx + w + 1];
            let gy_r = -r_chan[idx - w - 1] - 2.0 * r_chan[idx - w] - r_chan[idx - w + 1]
                + r_chan[idx + w - 1]
                + 2.0 * r_chan[idx + w]
                + r_chan[idx + w + 1];
            let mag_r = (gx_r * gx_r + gy_r * gy_r).sqrt();
            let edge_r = if mag_r < threshold_10 {
                0.0
            } else {
                mag_r.min(255.0)
            };
            let final_r = if invert {
                (255.0 - edge_r).round() as u8
            } else {
                edge_r.round() as u8
            };

            let gx_g = -g_chan[idx - w - 1] + g_chan[idx - w + 1] - 2.0 * g_chan[idx - 1]
                + 2.0 * g_chan[idx + 1]
                - g_chan[idx + w - 1]
                + g_chan[idx + w + 1];
            let gy_g = -g_chan[idx - w - 1] - 2.0 * g_chan[idx - w] - g_chan[idx - w + 1]
                + g_chan[idx + w - 1]
                + 2.0 * g_chan[idx + w]
                + g_chan[idx + w + 1];
            let mag_g = (gx_g * gx_g + gy_g * gy_g).sqrt();
            let edge_g = if mag_g < threshold_10 {
                0.0
            } else {
                mag_g.min(255.0)
            };
            let final_g = if invert {
                (255.0 - edge_g).round() as u8
            } else {
                edge_g.round() as u8
            };

            let gx_b = -b_chan[idx - w - 1] + b_chan[idx - w + 1] - 2.0 * b_chan[idx - 1]
                + 2.0 * b_chan[idx + 1]
                - b_chan[idx + w - 1]
                + b_chan[idx + w + 1];
            let gy_b = -b_chan[idx - w - 1] - 2.0 * b_chan[idx - w] - b_chan[idx - w + 1]
                + b_chan[idx + w - 1]
                + 2.0 * b_chan[idx + w]
                + b_chan[idx + w + 1];
            let mag_b = (gx_b * gx_b + gy_b * gy_b).sqrt();
            let edge_b = if mag_b < threshold_10 {
                0.0
            } else {
                mag_b.min(255.0)
            };
            let final_b = if invert {
                (255.0 - edge_b).round() as u8
            } else {
                edge_b.round() as u8
            };

            let out_idx = x * 4;
            row[out_idx] = final_r;
            row[out_idx + 1] = final_g;
            row[out_idx + 2] = final_b;
        }
    });

    img
}
