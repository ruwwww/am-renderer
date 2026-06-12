use image::RgbaImage;

pub fn apply_luma_key(img: RgbaImage, _low_threshold: f32, _high_threshold: f32) -> RgbaImage {
    eprintln!("Warning: LumaKey effect not yet implemented");
    img
}