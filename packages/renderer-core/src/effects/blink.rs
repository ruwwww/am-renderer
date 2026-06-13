use image::RgbaImage;

pub fn apply_blink(img: RgbaImage, _freq: f32, _time_secs: f32) -> RgbaImage {
    eprintln!("Warning: Blink effect not yet implemented as pixel effect");
    img
}
