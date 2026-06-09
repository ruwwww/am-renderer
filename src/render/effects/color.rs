use image::{RgbaImage, Rgba};
use rayon::prelude::*;

pub fn apply_exposure(img: &mut RgbaImage, exposure: f32) {
    let multiplier = 2.0_f32.powf(exposure);
    for pixel in img.pixels_mut() {
        pixel[0] = ((pixel[0] as f32 * multiplier).round().clamp(0.0, 255.0)) as u8;
        pixel[1] = ((pixel[1] as f32 * multiplier).round().clamp(0.0, 255.0)) as u8;
        pixel[2] = ((pixel[2] as f32 * multiplier).round().clamp(0.0, 255.0)) as u8;
    }
}

pub fn apply_brightness_contrast(img: &mut RgbaImage, brightness: f32, contrast: f32) {
    let bright_offset = brightness * 255.0;
    let contrast_factor = if contrast >= 0.0 {
        1.0 / (1.0 - contrast.min(0.99))
    } else {
        1.0 + contrast
    };

    for pixel in img.pixels_mut() {
        for c in 0..3 {
            let v = pixel[c] as f32;
            let v = (v - 128.0) * contrast_factor + 128.0 + bright_offset;
            pixel[c] = v.round().clamp(0.0, 255.0) as u8;
        }
    }
}

pub fn apply_hsl(img: &mut RgbaImage, hue_shift: f32, saturation: f32, lightness: f32) {
    for pixel in img.pixels_mut() {
        let r = pixel[0] as f32 / 255.0;
        let g = pixel[1] as f32 / 255.0;
        let b = pixel[2] as f32 / 255.0;

        let (h, s, l) = rgb_to_hsl(r, g, b);

        let h = (h + hue_shift) % 360.0;
        let h = if h < 0.0 { h + 360.0 } else { h };
        let s = (s + saturation).clamp(0.0, 1.0);
        let l = (l + lightness).clamp(0.0, 1.0);

        let (r2, g2, b2) = hsl_to_rgb(h, s, l);

        pixel[0] = (r2 * 255.0).round().clamp(0.0, 255.0) as u8;
        pixel[1] = (g2 * 255.0).round().clamp(0.0, 255.0) as u8;
        pixel[2] = (b2 * 255.0).round().clamp(0.0, 255.0) as u8;
    }
}

pub fn apply_vignette(
    img: &mut RgbaImage,
    feather: f32,
    roundness: f32,
    scale: f32,
    strength: f32,
    tint: f32,
    overlaycolor: [f32; 4],
    punchout: bool,
) {
    let w = img.width();
    let h = img.height();
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    println!("DEBUG: apply_vignette: w={}, h={}, feather={}, roundness={}, scale={}, strength={}, tint={}, punchout={}", w, h, feather, roundness, scale, strength, tint, punchout);

    let w_u = w as usize;
    let stride = w_u * 4;
    let raw = img.as_mut();

    let overlay_r = (overlaycolor[0] * 255.0).round().clamp(0.0, 255.0) as u8;
    let overlay_g = (overlaycolor[1] * 255.0).round().clamp(0.0, 255.0) as u8;
    let overlay_b = (overlaycolor[2] * 255.0).round().clamp(0.0, 255.0) as u8;

    let feather = feather.max(0.001);
    let scale = scale.max(0.001);

    raw.par_chunks_mut(stride)
        .enumerate()
        .for_each(|(y, row)| {
            let dy = y as f32 - cy;
            let ny = dy.abs() / cy.max(1.0);

            for x in 0..w_u {
                let dx = x as f32 - cx;
                let nx = dx.abs() / cx.max(1.0);

                // Compute distance based on roundness
                let d_rect = nx.max(ny);
                let d_circ = (nx * nx + ny * ny).sqrt();
                let d = ((1.0 - roundness) * d_rect + roundness * d_circ) / scale.max(0.001);

                // Compute vignette factor V (1.0 in center, decreasing towards corners)
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
                    // Punchout: keep center opaque (vignette near 1.0), make corners transparent (vignette near 0.0)
                    let original_a = row[idx + 3] as f32 / 255.0;
                    let final_a = (original_a * vignette * 255.0).round().clamp(0.0, 255.0) as u8;
                    row[idx + 3] = final_a;
                } else {
                    // Standard vignette: blend with overlaycolor using (1.0 - vignette) * tint
                    let blend_factor = (1.0 - vignette) * tint;

                    let r = row[idx] as f32 * (1.0 - blend_factor) + overlay_r as f32 * blend_factor;
                    let g = row[idx + 1] as f32 * (1.0 - blend_factor) + overlay_g as f32 * blend_factor;
                    let b = row[idx + 2] as f32 * (1.0 - blend_factor) + overlay_b as f32 * blend_factor;

                    row[idx] = r.round().clamp(0.0, 255.0) as u8;
                    row[idx + 1] = g.round().clamp(0.0, 255.0) as u8;
                    row[idx + 2] = b.round().clamp(0.0, 255.0) as u8;
                }
            }
        });
}

pub fn apply_color_fill(img: &mut RgbaImage, color: [f32; 4], opacity: f32) {
    let fill = Rgba([
        (color[0] * 255.0).round() as u8,
        (color[1] * 255.0).round() as u8,
        (color[2] * 255.0).round() as u8,
        255,
    ]);

    let opa = opacity * color[3];
    for pixel in img.pixels_mut() {
        if pixel[3] == 0 {
            continue;
        }
        for c in 0..3 {
            let src = fill[c] as f32;
            let dst = pixel[c] as f32;
            pixel[c] = (dst * (1.0 - opa) + src * opa).round().clamp(0.0, 255.0) as u8;
        }
    }
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < f32::EPSILON {
        let mut h = (g - b) / d;
        if g < b {
            h += 6.0;
        }
        h
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };

    (h * 60.0, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s.abs() < f32::EPSILON {
        return (l, l, l);
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let h_norm = h / 360.0;

    let r = hue_to_rgb(p, q, h_norm + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h_norm);
    let b = hue_to_rgb(p, q, h_norm - 1.0 / 3.0);

    (r, g, b)
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

pub fn find_edges(img: &mut RgbaImage, _smoothing: f32, threshold: f32, invert: bool) {
    let w = img.width() as usize;
    let h = img.height() as usize;

    if w < 3 || h < 3 {
        return;
    }

    let mut r_chan = vec![0.0f32; w * h];
    let mut g_chan = vec![0.0f32; w * h];
    let mut b_chan = vec![0.0f32; w * h];

    let w_u = w;
    let stride = w_u * 4;
    let raw_src = img.as_raw();

    r_chan.par_chunks_mut(w_u)
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

    raw.par_chunks_mut(stride)
        .enumerate()
        .for_each(|(y, row)| {
            if y == 0 || y == h - 1 {
                return;
            }
            for x in 1..(w - 1) {
                let idx = y * w + x;

                // Sobel for Red
                let gx_r = -r_chan[idx - w - 1] + r_chan[idx - w + 1]
                           - 2.0 * r_chan[idx - 1] + 2.0 * r_chan[idx + 1]
                           - r_chan[idx + w - 1] + r_chan[idx + w + 1];
                let gy_r = -r_chan[idx - w - 1] - 2.0 * r_chan[idx - w] - r_chan[idx - w + 1]
                           + r_chan[idx + w - 1] + 2.0 * r_chan[idx + w] + r_chan[idx + w + 1];
                let mag_r = (gx_r * gx_r + gy_r * gy_r).sqrt();
                let edge_r = if mag_r < threshold_10 { 0.0 } else { mag_r.min(255.0) };
                let final_r = if invert { (255.0 - edge_r).round() as u8 } else { edge_r.round() as u8 };

                // Sobel for Green
                let gx_g = -g_chan[idx - w - 1] + g_chan[idx - w + 1]
                           - 2.0 * g_chan[idx - 1] + 2.0 * g_chan[idx + 1]
                           - g_chan[idx + w - 1] + g_chan[idx + w + 1];
                let gy_g = -g_chan[idx - w - 1] - 2.0 * g_chan[idx - w] - g_chan[idx - w + 1]
                           + g_chan[idx + w - 1] + 2.0 * g_chan[idx + w] + g_chan[idx + w + 1];
                let mag_g = (gx_g * gx_g + gy_g * gy_g).sqrt();
                let edge_g = if mag_g < threshold_10 { 0.0 } else { mag_g.min(255.0) };
                let final_g = if invert { (255.0 - edge_g).round() as u8 } else { edge_g.round() as u8 };

                // Sobel for Blue
                let gx_b = -b_chan[idx - w - 1] + b_chan[idx - w + 1]
                           - 2.0 * b_chan[idx - 1] + 2.0 * b_chan[idx + 1]
                           - b_chan[idx + w - 1] + b_chan[idx + w + 1];
                let gy_b = -b_chan[idx - w - 1] - 2.0 * b_chan[idx - w] - b_chan[idx - w + 1]
                           + b_chan[idx + w - 1] + 2.0 * b_chan[idx + w] + b_chan[idx + w + 1];
                let mag_b = (gx_b * gx_b + gy_b * gy_b).sqrt();
                let edge_b = if mag_b < threshold_10 { 0.0 } else { mag_b.min(255.0) };
                let final_b = if invert { (255.0 - edge_b).round() as u8 } else { edge_b.round() as u8 };

                let out_idx = x * 4;
                row[out_idx] = final_r;
                row[out_idx + 1] = final_g;
                row[out_idx + 2] = final_b;
            }
        });
}