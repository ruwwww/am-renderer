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

pub fn apply_vignette(img: &mut RgbaImage, intensity: f32, radius: f32) {
    let w = img.width() as f32;
    let h = img.height() as f32;
    let cx = w / 2.0;
    let cy = h / 2.0;
    let max_dist = (cx * cx + cy * cy).sqrt();

    let w_u = img.width() as usize;
    let stride = w_u * 4;
    let raw = img.as_mut();

    raw.par_chunks_mut(stride)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w_u {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt() / max_dist;

                let vignette = if dist < radius {
                    1.0
                } else {
                    let t = ((dist - radius) / (1.0 - radius).max(0.001)).clamp(0.0, 1.0);
                    1.0 - t * intensity
                };

                let idx = x * 4;
                row[idx] = ((row[idx] as f32 * vignette).round().clamp(0.0, 255.0)) as u8;
                row[idx + 1] = ((row[idx + 1] as f32 * vignette).round().clamp(0.0, 255.0)) as u8;
                row[idx + 2] = ((row[idx + 2] as f32 * vignette).round().clamp(0.0, 255.0)) as u8;
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

    let gray: Vec<f32> = (0..h).into_par_iter().flat_map(|y| {
        let mut row = Vec::with_capacity(w);
        for x in 0..w {
            let p = img.get_pixel(x as u32, y as u32);
            row.push(0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32);
        }
        row
    }).collect();

    let threshold_10 = threshold * 10.0;
    let stride = w * 4;
    let raw = img.as_mut();

    raw.par_chunks_mut(stride)
        .enumerate()
        .for_each(|(y, row)| {
            if y == 0 || y == h - 1 {
                return;
            }
            for x in 1..(w - 1) {
                let idx = y * w + x;
                let tl = gray[idx - w - 1];
                let tc = gray[idx - w];
                let tr = gray[idx - w + 1];
                let ml = gray[idx - 1];
                let mr = gray[idx + 1];
                let bl = gray[idx + w - 1];
                let bc = gray[idx + w];
                let br = gray[idx + w + 1];

                let gx = -tl + tr - 2.0 * ml + 2.0 * mr - bl + br;
                let gy = -tl - 2.0 * tc - tr + bl + 2.0 * bc + br;

                let magnitude = (gx * gx + gy * gy).sqrt();
                let edge_val = if magnitude < threshold_10 {
                    0.0
                } else {
                    magnitude.min(255.0)
                };

                let final_val = if invert {
                    (255.0 - edge_val).round() as u8
                } else {
                    edge_val.round() as u8
                };

                let out_idx = x * 4;
                row[out_idx] = final_val;
                row[out_idx + 1] = final_val;
                row[out_idx + 2] = final_val;
            }
        });
}