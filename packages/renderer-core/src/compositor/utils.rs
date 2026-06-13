use image::Rgba;

/// Convert RGBA [f32; 4] (0.0 - 1.0) to image::Rgba<u8>.
pub(crate) fn to_rgba_u8(color: [f32; 4]) -> Rgba<u8> {
    Rgba([
        (color[0] * 255.0).round().clamp(0.0, 255.0) as u8,
        (color[1] * 255.0).round().clamp(0.0, 255.0) as u8,
        (color[2] * 255.0).round().clamp(0.0, 255.0) as u8,
        (color[3] * 255.0).round().clamp(0.0, 255.0) as u8,
    ])
}

