use image::RgbaImage;

pub fn apply_motion_blur(img: RgbaImage, _tune: f32, _time_secs: f32, _prev_location: [f32; 3]) -> RgbaImage {
    eprintln!("Warning: MotionBlur effect not yet implemented");
    img
}