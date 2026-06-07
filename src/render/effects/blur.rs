//! Blur effect implementations — Gaussian blur and box blur.
//!
//! Both use separable horizontal+vertical passes for efficiency.

use image::{RgbaImage, Rgba};

/// Apply a Gaussian blur to an image.
///
/// Uses a separable two-pass approach (horizontal then vertical) for
/// O(n*r) performance instead of O(n*r²).
///
/// # Arguments
/// * `img` - Source image
/// * `radius` - Blur radius in pixels (clamped to reasonable max)
///
/// # Returns
/// New blurred image.
pub fn gaussian_blur(img: &RgbaImage, radius: f32) -> RgbaImage {
    let radius = radius.max(0.0).min(200.0);
    if radius < 0.5 {
        return img.clone();
    }

    let kernel = build_gaussian_kernel(radius);
    let half = (kernel.len() / 2) as i32;

    // Horizontal pass
    let intermediate = blur_pass(img, &kernel, half, true);
    // Vertical pass
    blur_pass(&intermediate, &kernel, half, false)
}

/// Apply a box blur to an image (fast approximation of Gaussian blur).
///
/// Uses a separable two-pass approach with a uniform kernel.
///
/// # Arguments
/// * `img` - Source image
/// * `radius` - Blur radius in pixels
///
/// # Returns
/// New blurred image.
pub fn box_blur(img: &RgbaImage, radius: u32) -> RgbaImage {
    if radius == 0 {
        return img.clone();
    }

    let radius = radius.min(200);
    let size = (2 * radius + 1) as usize;
    let weight = 1.0 / size as f32;
    let kernel: Vec<f32> = vec![weight; size];
    let half = radius as i32;

    let intermediate = blur_pass(img, &kernel, half, true);
    blur_pass(&intermediate, &kernel, half, false)
}

/// Build a 1D Gaussian kernel for the given radius.
///
/// The kernel size is `2 * ceil(radius * 2) + 1` to capture ~95% of the distribution.
fn build_gaussian_kernel(radius: f32) -> Vec<f32> {
    let sigma = radius / 2.0;
    let kernel_radius = (radius * 2.0).ceil() as i32;
    let size = (2 * kernel_radius + 1) as usize;
    let mut kernel = Vec::with_capacity(size);
    let mut sum = 0.0;

    for i in -kernel_radius..=kernel_radius {
        let x = i as f32;
        let g = (-x * x / (2.0 * sigma * sigma)).exp();
        kernel.push(g);
        sum += g;
    }

    // Normalize
    for v in &mut kernel {
        *v /= sum;
    }

    kernel
}

/// Single-pass separable blur (horizontal or vertical).
fn blur_pass(img: &RgbaImage, kernel: &[f32], half: i32, horizontal: bool) -> RgbaImage {
    let w = img.width() as i32;
    let h = img.height() as i32;
    let mut result = RgbaImage::new(w as u32, h as u32);

    for y in 0..h {
        for x in 0..w {
            let mut r = 0.0_f32;
            let mut g = 0.0_f32;
            let mut b = 0.0_f32;
            let mut a = 0.0_f32;

            for (ki, kv) in kernel.iter().enumerate() {
                let offset = ki as i32 - half;
                let (sx, sy) = if horizontal {
                    ((x + offset).clamp(0, w - 1), y)
                } else {
                    (x, (y + offset).clamp(0, h - 1))
                };

                let pixel = img.get_pixel(sx as u32, sy as u32);
                r += pixel[0] as f32 * kv;
                g += pixel[1] as f32 * kv;
                b += pixel[2] as f32 * kv;
                a += pixel[3] as f32 * kv;
            }

            result.put_pixel(
                x as u32,
                y as u32,
                Rgba([
                    r.round().clamp(0.0, 255.0) as u8,
                    g.round().clamp(0.0, 255.0) as u8,
                    b.round().clamp(0.0, 255.0) as u8,
                    a.round().clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }

    result
}
