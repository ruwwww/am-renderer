use image::RgbaImage;

pub fn apply_exposure(img: RgbaImage, exposure: f32, gamma: f32, offset: f32) -> RgbaImage {
    let mut img = img;
    let multiplier = 2.0_f32.powf(exposure);
    for pixel in img.pixels_mut() {
        for c in 0..3 {
            let val = pixel[c] as f32 / 255.0;
            // 1. Exposure stops multiplier
            let val = val * multiplier;
            // 2. Gamma correction
            let val = if val > 0.0 { val.powf(gamma) } else { 0.0 };
            // 3. Additive offset
            let val = val + offset;

            pixel[c] = (val.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    img
}
