use image::RgbaImage;

pub fn apply_gradient_overlay(img: RgbaImage, _alpha: f32, _color1: [f32; 4], _color2: [f32; 4], _offset: [f32; 2], _scale: f32) -> RgbaImage {
    eprintln!("Warning: GradientOverlay effect not yet implemented");
    img
}