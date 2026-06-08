//! Color effect implementations — exposure, brightness/contrast, saturation, vignette.
//!
//! These effects operate on pixel buffers and are applied after layer source
//! generation but before compositing onto the canvas.

use image::{RgbaImage, Rgba};

/// Apply exposure adjustment to an image buffer.
///
/// Exposure is measured in EV stops: each +1.0 doubles brightness.
///
/// # Arguments
/// * `img` - Source image
/// * `exposure` - Exposure value in EV stops
///
/// # Returns
/// New image with exposure applied.
pub fn apply_exposure(img: &RgbaImage, exposure: f32) -> RgbaImage {
    let multiplier = 2.0_f32.powf(exposure);
    let mut result = img.clone();
    for pixel in result.pixels_mut() {
        pixel[0] = ((pixel[0] as f32 * multiplier).round().clamp(0.0, 255.0)) as u8;
        pixel[1] = ((pixel[1] as f32 * multiplier).round().clamp(0.0, 255.0)) as u8;
        pixel[2] = ((pixel[2] as f32 * multiplier).round().clamp(0.0, 255.0)) as u8;
        // Alpha unchanged
    }
    result
}

/// Apply brightness and contrast adjustment.
///
/// Brightness is added to each channel. Contrast scales around the midpoint (128).
///
/// # Arguments
/// * `img` - Source image
/// * `brightness` - Brightness adjustment (-1.0 to 1.0, mapped to -255 to +255)
/// * `contrast` - Contrast adjustment (-1.0 to 1.0, mapped to scale factor)
///
/// # Returns
/// New image with brightness/contrast applied.
pub fn apply_brightness_contrast(img: &RgbaImage, brightness: f32, contrast: f32) -> RgbaImage {
    let bright_offset = brightness * 255.0;
    // Contrast: map [-1, 1] to [0, 2] scale factor around midpoint
    let contrast_factor = if contrast >= 0.0 {
        1.0 / (1.0 - contrast.min(0.99))
    } else {
        1.0 + contrast
    };

    let mut result = img.clone();
    for pixel in result.pixels_mut() {
        for c in 0..3 {
            let v = pixel[c] as f32;
            let v = (v - 128.0) * contrast_factor + 128.0 + bright_offset;
            pixel[c] = v.round().clamp(0.0, 255.0) as u8;
        }
    }
    result
}

/// Apply hue/saturation/lightness adjustment.
///
/// Converts each pixel to HSL, applies modifications, and converts back to RGB.
///
/// # Arguments
/// * `img` - Source image
/// * `hue_shift` - Hue shift in degrees
/// * `saturation` - Saturation adjustment (-1.0 to 1.0)
/// * `lightness` - Lightness adjustment (-1.0 to 1.0)
///
/// # Returns
/// New image with HSL adjustments applied.
pub fn apply_hsl(img: &RgbaImage, hue_shift: f32, saturation: f32, lightness: f32) -> RgbaImage {
    let mut result = img.clone();
    for pixel in result.pixels_mut() {
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
    result
}

/// Apply vignette effect (darkens edges of the image).
///
/// # Arguments
/// * `img` - Source image
/// * `intensity` - How dark the vignette is (0.0 = none, 1.0 = fully dark at edges)
/// * `radius` - Vignette inner radius (0.0 = all dark, 1.0 = vignette starts at edge)
///
/// # Returns
/// New image with vignette applied.
pub fn apply_vignette(img: &RgbaImage, intensity: f32, radius: f32) -> RgbaImage {
    let w = img.width() as f32;
    let h = img.height() as f32;
    let cx = w / 2.0;
    let cy = h / 2.0;
    let max_dist = (cx * cx + cy * cy).sqrt();

    let mut result = img.clone();
    for y in 0..img.height() {
        for x in 0..img.width() {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt() / max_dist;

            let vignette = if dist < radius {
                1.0
            } else {
                let t = ((dist - radius) / (1.0 - radius).max(0.001)).clamp(0.0, 1.0);
                1.0 - t * intensity
            };

            let pixel = result.get_pixel_mut(x, y);
            for c in 0..3 {
                pixel[c] = ((pixel[c] as f32 * vignette).round().clamp(0.0, 255.0)) as u8;
            }
        }
    }
    result
}

/// Apply a solid color fill overlay.
///
/// # Arguments
/// * `img` - Source image
/// * `color` - Overlay color as RGBA [f32; 4]
/// * `opacity` - Overlay opacity (0.0 - 1.0)
///
/// # Returns
/// New image with color fill overlaid.
pub fn apply_color_fill(img: &RgbaImage, color: [f32; 4], opacity: f32) -> RgbaImage {
    let mut result = img.clone();
    let fill = Rgba([
        (color[0] * 255.0).round() as u8,
        (color[1] * 255.0).round() as u8,
        (color[2] * 255.0).round() as u8,
        255,
    ]);

    let opa = opacity * color[3];
    for pixel in result.pixels_mut() {
        if pixel[3] == 0 {
            continue; // Don't fill transparent areas
        }
        for c in 0..3 {
            let src = fill[c] as f32;
            let dst = pixel[c] as f32;
            pixel[c] = (dst * (1.0 - opa) + src * opa).round().clamp(0.0, 255.0) as u8;
        }
    }
    result
}

// ── HSL conversion helpers ───────────────────────────────────────

/// Convert RGB (0.0-1.0) to HSL. Returns (hue: 0-360, saturation: 0-1, lightness: 0-1).
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

/// Convert HSL to RGB. All values in standard ranges.
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

/// Helper for HSL→RGB conversion.
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

/// Apply edge detection (Sobel filter) to an image.
///
/// # Arguments
/// * `img` - Source image
/// * `smoothing` - Blend/smooth factor
/// * `threshold` - Edge threshold
/// * `invert` - If true, returns dark edges on white, else white edges on black.
pub fn find_edges(img: &RgbaImage, _smoothing: f32, threshold: f32, invert: bool) -> RgbaImage {
    let w = img.width();
    let h = img.height();
    let mut result = img.clone();

    if w < 3 || h < 3 {
        return result;
    }

    // We compute grayscale values first for convenience
    let mut gray = vec![0.0f32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x, y);
            gray[(y * w + x) as usize] = 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32;
        }
    }

    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let mut gx = 0.0;
            let mut gy = 0.0;

            // Apply Sobel kernels
            for ky in 0..3 {
                for kx in 0..3 {
                    let px = (x as i32 + kx as i32 - 1) as u32;
                    let py = (y as i32 + ky as i32 - 1) as u32;
                    let val = gray[(py * w + px) as usize];

                    // gx kernel:
                    // [-1  0  1]
                    // [-2  0  2]
                    // [-1  0  1]
                    let k_gx = match (kx, ky) {
                        (0, 0) | (0, 2) => -1.0,
                        (0, 1) => -2.0,
                        (2, 0) | (2, 2) => 1.0,
                        (2, 1) => 2.0,
                        _ => 0.0,
                    };
                    gx += val * k_gx;

                    // gy kernel:
                    // [-1 -2 -1]
                    // [ 0  0  0]
                    // [ 1  2  1]
                    let k_gy = match (kx, ky) {
                        (0, 0) | (2, 0) => -1.0,
                        (1, 0) => -2.0,
                        (0, 2) | (2, 2) => 1.0,
                        (1, 2) => 2.0,
                        _ => 0.0,
                    };
                    gy += val * k_gy;
                }
            }

            let magnitude = (gx * gx + gy * gy).sqrt();
            let edge_val = if magnitude < threshold * 10.0 {
                0.0
            } else {
                magnitude.min(255.0)
            };

            let final_val = if invert {
                255 - edge_val.round() as u8
            } else {
                edge_val.round() as u8
            };

            let pixel = result.get_pixel_mut(x, y);
            pixel[0] = final_val;
            pixel[1] = final_val;
            pixel[2] = final_val;
        }
    }

    result
}
