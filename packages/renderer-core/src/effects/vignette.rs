use image::RgbaImage;
use rayon::prelude::*;

pub fn apply_vignette(
    img: RgbaImage,
    feather: f32,
    roundness: f32,
    scale: f32,
    strength: f32,
    tint: f32,
    overlaycolor: [f32; 4],
    punchout: bool,
) -> RgbaImage {
    let mut img = img;
    let w = img.width();
    let h = img.height();
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;

    let w_u = w as usize;
    let stride = w_u * 4;
    let raw = img.as_mut();

    let overlay_r = (overlaycolor[0] * 255.0).round().clamp(0.0, 255.0) as u8;
    let overlay_g = (overlaycolor[1] * 255.0).round().clamp(0.0, 255.0) as u8;
    let overlay_b = (overlaycolor[2] * 255.0).round().clamp(0.0, 255.0) as u8;

    let feather = feather.max(0.001);
    let scale = scale.max(0.001);

    let (rx, ry) = if cx < cy {
        (cx, cy + (cx - cy) * roundness)
    } else {
        (cx + (cy - cx) * roundness, cy)
    };
    let rx = rx.max(1.0);
    let ry = ry.max(1.0);

    raw.par_chunks_mut(stride).enumerate().for_each(|(y, row)| {
        let dy = y as f32 - cy;

        for x in 0..w_u {
            let dx = x as f32 - cx;

            let nx = dx / rx;
            let ny = dy / ry;
            let d = (nx * nx + ny * ny).sqrt() / scale;

            let vignette = if d < 1.0 - feather {
                1.0
            } else if d > 1.0 {
                1.0 - strength
            } else {
                let t = (d - (1.0 - feather)) / feather;
                1.0 - t * strength
            };

            let idx = x * 4;

            if punchout {
                let original_a = row[idx + 3] as f32 / 255.0;
                let final_a = (original_a * vignette * 255.0).round().clamp(0.0, 255.0) as u8;
                row[idx + 3] = final_a;
            } else {
                let blend_factor = (1.0 - vignette) * tint;

                let r = row[idx] as f32 * (1.0 - blend_factor) + overlay_r as f32 * blend_factor;
                let g =
                    row[idx + 1] as f32 * (1.0 - blend_factor) + overlay_g as f32 * blend_factor;
                let b =
                    row[idx + 2] as f32 * (1.0 - blend_factor) + overlay_b as f32 * blend_factor;

                row[idx] = r.round().clamp(0.0, 255.0) as u8;
                row[idx + 1] = g.round().clamp(0.0, 255.0) as u8;
                row[idx + 2] = b.round().clamp(0.0, 255.0) as u8;
            }
        }
    });

    img
}
