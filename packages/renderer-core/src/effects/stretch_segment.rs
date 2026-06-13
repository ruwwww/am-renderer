use image::RgbaImage;
use rayon::prelude::*;

pub fn apply_stretch_segment(
    img: RgbaImage,
    angle: f32,
    stretch: f32,
    offset: f32,
    smooth: f32,
) -> RgbaImage {
    let w = img.width() as usize;
    let h = img.height() as usize;
    let mut result = RgbaImage::new(w as u32, h as u32);

    let angle_rad = angle.to_radians();
    let cx = w as f32 / 2.0 + offset * angle_rad.sin();
    let cy = h as f32 / 2.0 + offset * angle_rad.cos();
    let half_stretch = stretch / 2.0;
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();
    let w_m1 = w as f32 - 1.0;
    let h_m1 = h as f32 - 1.0;

    let src_raw = img.as_raw();
    let stride = w * 4;
    let dst_raw = result.as_mut();

    let smooth_scale_inside = smooth.clamp(0.0, 1.0);
    let smooth_scale_outside = 1.0 - smooth_scale_inside;

    dst_raw
        .par_chunks_mut(stride)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let proj = dx * cos_a + dy * sin_a;

                let (sx, sy) = if proj > half_stretch {
                    let src_x = x as f32 - cos_a * half_stretch * smooth_scale_outside;
                    let src_y = y as f32 - sin_a * half_stretch * smooth_scale_outside;
                    (
                        src_x.round().clamp(0.0, w_m1) as usize,
                        src_y.round().clamp(0.0, h_m1) as usize,
                    )
                } else if proj < -half_stretch {
                    let src_x = x as f32 + cos_a * half_stretch * smooth_scale_outside;
                    let src_y = y as f32 + sin_a * half_stretch * smooth_scale_outside;
                    (
                        src_x.round().clamp(0.0, w_m1) as usize,
                        src_y.round().clamp(0.0, h_m1) as usize,
                    )
                } else {
                    let src_x = x as f32 - proj * cos_a * smooth_scale_outside;
                    let src_y = y as f32 - proj * sin_a * smooth_scale_outside;
                    (
                        src_x.round().clamp(0.0, w_m1) as usize,
                        src_y.round().clamp(0.0, h_m1) as usize,
                    )
                };

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
