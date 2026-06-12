use image::RgbaImage;
use anyhow::Result;

use crate::eval::transform::transform_point;

pub fn apply_lift(
    w: u32,
    h: u32,
    layer_w: f32,
    layer_h: f32,
    fill: f32,
    shape_img: Option<RgbaImage>,
    canvas: &RgbaImage,
    fwd: &[[f32; 3]; 3],
) -> Result<RgbaImage> {
    let mut bg_img = RgbaImage::new(w, h);
    let half_w = layer_w / 2.0;
    let half_h = layer_h / 2.0;
    let cw = canvas.width() as i32;
    let ch = canvas.height() as i32;

    let dx_per_px = layer_w / w as f32;
    let dy_per_py = layer_h / h as f32;

    let fwd_step_x = [fwd[0][0] * dx_per_px, fwd[0][1] * dx_per_px];
    let fwd_step_y = [fwd[1][0] * dy_per_py, fwd[1][1] * dy_per_py];

    let local_x0 = 0.5 / w as f32 * layer_w - half_w;
    let local_y0 = 0.5 / h as f32 * layer_h - half_h;
    let canvas_origin = transform_point(fwd, [local_x0, local_y0]);

    let fill_f = fill.clamp(0.0, 1.0);
    let fill_f_u = (fill_f * 256.0) as u32;
    let inv_f_u = 256 - fill_f_u;

    let canvas_raw = canvas.as_raw();
    let canvas_stride = cw as usize * 4;

    let bg_raw = bg_img.as_mut();

    for ly in 0..h {
        let row_cx0 = canvas_origin[0] + fwd_step_y[0] * ly as f32;
        let row_cy0 = canvas_origin[1] + fwd_step_y[1] * ly as f32;

        for lx in 0..w {
            let cx = (row_cx0 + fwd_step_x[0] * lx as f32) as i32;
            let cy = (row_cy0 + fwd_step_x[1] * lx as f32) as i32;

            let bg_pixel = if cx >= 0 && cx < cw && cy >= 0 && cy < ch {
                let off = cy as usize * canvas_stride + cx as usize * 4;
                [canvas_raw[off], canvas_raw[off+1], canvas_raw[off+2], canvas_raw[off+3]]
            } else {
                [0u8; 4]
            };

            let dst_off = (ly * w + lx) as usize * 4;
            let final_px = if let Some(ref s_img) = shape_img {
                let sp = s_img.get_pixel(lx, ly).0;
                [
                    ((bg_pixel[0] as u32 * inv_f_u + sp[0] as u32 * fill_f_u) >> 8) as u8,
                    ((bg_pixel[1] as u32 * inv_f_u + sp[1] as u32 * fill_f_u) >> 8) as u8,
                    ((bg_pixel[2] as u32 * inv_f_u + sp[2] as u32 * fill_f_u) >> 8) as u8,
                    ((bg_pixel[3] as u32 * inv_f_u + sp[3] as u32 * fill_f_u) >> 8) as u8,
                ]
            } else {
                bg_pixel
            };

            bg_raw[dst_off]     = final_px[0];
            bg_raw[dst_off + 1] = final_px[1];
            bg_raw[dst_off + 2] = final_px[2];
            bg_raw[dst_off + 3] = final_px[3];
        }
    }

    Ok(bg_img)
}