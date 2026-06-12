use image::RgbaImage;

pub fn apply_lens_blur(img: RgbaImage, _radius: f32, _strength: f32) -> RgbaImage {
    eprintln!("Warning: LensBlur effect not yet implemented");
    img
}