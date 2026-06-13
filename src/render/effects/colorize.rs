use image::RgbaImage;

/// Returns the pure RGB color for a given hue in degrees [0, 360] at max saturation and value.
fn hue_to_rgb(hue: f32) -> [f32; 3] {
    // Normalize hue to [0, 360)
    let h = hue.rem_euclid(360.0);
    let x = (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs()).max(0.0).min(1.0);

    if h < 60.0 {
        [1.0, x, 0.0]
    } else if h < 120.0 {
        [x, 1.0, 0.0]
    } else if h < 180.0 {
        [0.0, 1.0, x]
    } else if h < 240.0 {
        [0.0, x, 1.0]
    } else if h < 300.0 {
        [x, 0.0, 1.0]
    } else {
        [1.0, 0.0, x]
    }
}

pub fn apply_colorize(img: RgbaImage, hue: f32, strength: f32) -> RgbaImage {
    let mut img = img;
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.001 {
        return img;
    }

    // Get the base RGB for the target hue
    let rgb_hue = hue_to_rgb(hue);
    // Calculate the luminance of the target hue using standard ITU-R BT.601 coefficients
    let y_hue = 0.299 * rgb_hue[0] + 0.587 * rgb_hue[1] + 0.114 * rgb_hue[2];
    
    // Fallback in case y_hue is somehow 0
    let y_hue = if y_hue > 0.0001 { y_hue } else { 1.0 };

    for pixel in img.pixels_mut() {
        // We only process RGB channels, keep Alpha intact
        let r_orig = pixel[0] as f32 / 255.0;
        let g_orig = pixel[1] as f32 / 255.0;
        let b_orig = pixel[2] as f32 / 255.0;

        // Calculate original luminance
        let y_orig = 0.299 * r_orig + 0.587 * g_orig + 0.114 * b_orig;

        // Scale the target hue to have the same luminance
        let r_target = rgb_hue[0] * (y_orig / y_hue);
        let g_target = rgb_hue[1] * (y_orig / y_hue);
        let b_target = rgb_hue[2] * (y_orig / y_hue);

        // Mix original and target colors based on strength
        let r_final = (1.0 - strength) * r_orig + strength * r_target;
        let g_final = (1.0 - strength) * g_orig + strength * g_target;
        let b_final = (1.0 - strength) * b_orig + strength * b_target;

        // Clamp to [0, 1] and write back
        pixel[0] = (r_final.clamp(0.0, 1.0) * 255.0).round() as u8;
        pixel[1] = (g_final.clamp(0.0, 1.0) * 255.0).round() as u8;
        pixel[2] = (b_final.clamp(0.0, 1.0) * 255.0).round() as u8;
    }

    img
}
