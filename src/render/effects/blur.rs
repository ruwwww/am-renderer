use image::RgbaImage;
use rayon::prelude::*;

pub fn gaussian_blur(img: &RgbaImage, radius: f32) -> RgbaImage {
    let radius = radius.max(0.0).min(200.0);
    if radius < 0.5 {
        return img.clone();
    }

    let kernel = build_gaussian_kernel(radius);
    let half = (kernel.len() / 2) as i32;

    let intermediate = blur_pass(img, &kernel, half, true);
    blur_pass(&intermediate, &kernel, half, false)
}

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

    for v in &mut kernel {
        *v /= sum;
    }

    kernel
}

fn blur_pass(img: &RgbaImage, kernel: &[f32], half: i32, horizontal: bool) -> RgbaImage {
    let w = img.width() as i32;
    let h = img.height() as i32;
    let mut result = RgbaImage::new(w as u32, h as u32);

    let w_u = w as usize;
    let stride = w_u * 4;
    let result_raw = result.as_mut();

    result_raw
        .par_chunks_mut(stride)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w {
                let mut r = 0.0_f32;
                let mut g = 0.0_f32;
                let mut b = 0.0_f32;
                let mut a = 0.0_f32;

                for (ki, kv) in kernel.iter().enumerate() {
                    let offset = ki as i32 - half;
                    let (sx, sy) = if horizontal {
                        ((x + offset).clamp(0, w - 1), y as i32)
                    } else {
                        (x, (y as i32 + offset).clamp(0, h - 1))
                    };

                    let pixel = img.get_pixel(sx as u32, sy as u32);
                    r += pixel[0] as f32 * kv;
                    g += pixel[1] as f32 * kv;
                    b += pixel[2] as f32 * kv;
                    a += pixel[3] as f32 * kv;
                }

                let dst_idx = x as usize * 4;
                row[dst_idx] = r.round().clamp(0.0, 255.0) as u8;
                row[dst_idx + 1] = g.round().clamp(0.0, 255.0) as u8;
                row[dst_idx + 2] = b.round().clamp(0.0, 255.0) as u8;
                row[dst_idx + 3] = a.round().clamp(0.0, 255.0) as u8;
            }
        });

    result
}