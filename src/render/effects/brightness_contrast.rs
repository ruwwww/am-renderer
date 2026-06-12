use image::RgbaImage;

pub fn apply_brightness_contrast(img: RgbaImage, brightness: f32, contrast: f32) -> RgbaImage {
    let mut img = img;
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
    img
}
