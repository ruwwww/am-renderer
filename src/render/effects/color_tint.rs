use image::{RgbaImage, Rgba};

pub fn apply_color_fill(img: RgbaImage, color: [f32; 4], opacity: f32) -> RgbaImage {
    let mut img = img;
    let fill = Rgba([
        (color[0] * 255.0).round() as u8,
        (color[1] * 255.0).round() as u8,
        (color[2] * 255.0).round() as u8,
        255,
    ]);

    let opa = opacity * color[3];
    for pixel in img.pixels_mut() {
        if pixel[3] == 0 {
            continue;
        }
        for c in 0..3 {
            let src = fill[c] as f32;
            let dst = pixel[c] as f32;
            pixel[c] = (dst * (1.0 - opa) + src * opa).round().clamp(0.0, 255.0) as u8;
        }
    }
    img
}
