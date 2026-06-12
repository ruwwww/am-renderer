use image::Rgba;
use log::warn;

/// Convert RGBA [f32; 4] (0.0 - 1.0) to image::Rgba<u8>.
pub(crate) fn to_rgba_u8(color: [f32; 4]) -> Rgba<u8> {
    Rgba([
        (color[0] * 255.0).round().clamp(0.0, 255.0) as u8,
        (color[1] * 255.0).round().clamp(0.0, 255.0) as u8,
        (color[2] * 255.0).round().clamp(0.0, 255.0) as u8,
        (color[3] * 255.0).round().clamp(0.0, 255.0) as u8,
    ])
}

/// Parse a hex color string in #AARRGGBB format to RGBA [f32; 4].
///
/// Handles both 8-char (#AARRGGBB) and 6-char (#RRGGBB, assumes full alpha) formats.
/// Note: Alight Motion uses ARGB order, not RGBA!
pub fn parse_hex_color(hex: &str) -> [f32; 4] {
    let hex = hex.trim_start_matches('#');

    match hex.len() {
        8 => {
            // #AARRGGBB → RGBA
            let a = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
            let r = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[6..8], 16).unwrap_or(0);
            [
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                a as f32 / 255.0,
            ]
        }
        6 => {
            // #RRGGBB → RGBA with full alpha
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
        }
        _ => {
            warn!("Invalid hex color format: #{}", hex);
            [0.0, 0.0, 0.0, 1.0]
        }
    }
}
