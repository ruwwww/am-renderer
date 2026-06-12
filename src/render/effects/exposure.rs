use image::RgbaImage;

pub fn apply_exposure(img: RgbaImage, exposure: f32) -> RgbaImage {
    let mut img = img;
    let multiplier = 2.0_f32.powf(exposure);
    for pixel in img.pixels_mut() {
        pixel[0] = ((pixel[0] as f32 * multiplier).round().clamp(0.0, 255.0)) as u8;
        pixel[1] = ((pixel[1] as f32 * multiplier).round().clamp(0.0, 255.0)) as u8;
        pixel[2] = ((pixel[2] as f32 * multiplier).round().clamp(0.0, 255.0)) as u8;
    }
    img
}
