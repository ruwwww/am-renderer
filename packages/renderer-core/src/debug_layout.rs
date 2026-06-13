//! Debug layout visualization utilities for the Alight Motion renderer.
//!
//! Provides line, rectangle, pixel font, and bounding-box outline drawing.

use graph_resolver::eval::transform::{build_transform_matrix, transform_point};
use graph_resolver::model::ResolvedLayer;
use crate::effects::transform::apply_transform_effects;
use image::{Rgba, RgbaImage};

pub fn draw_line(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    let mut x = x0;
    let mut y = y0;

    loop {
        if x >= 0 && x < img.width() as i32 && y >= 0 && y < img.height() as i32 {
            img.put_pixel(x as u32, y as u32, color);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

pub fn draw_rect(img: &mut RgbaImage, x: i32, y: i32, w: i32, h: i32, color: Rgba<u8>) {
    draw_line(img, x, y, x + w, y, color);
    draw_line(img, x + w, y, x + w, y + h, color);
    draw_line(img, x + w, y + h, x, y + h, color);
    draw_line(img, x, y + h, x, y, color);
}

fn get_char_pixels(c: char) -> u16 {
    match c.to_ascii_uppercase() {
        '0' => 0b111_101_101_101_111,
        '1' => 0b010_110_010_010_111,
        '2' => 0b111_001_111_100_111,
        '3' => 0b111_001_111_001_111,
        '4' => 0b101_101_111_001_001,
        '5' => 0b111_100_111_001_111,
        '6' => 0b111_100_111_101_111,
        '7' => 0b111_001_010_010_010,
        '8' => 0b111_101_111_101_111,
        '9' => 0b111_101_111_001_111,
        'A' => 0b111_101_111_101_101,
        'B' => 0b110_101_110_101_110,
        'C' => 0b111_100_100_100_111,
        'D' => 0b110_101_101_101_110,
        'E' => 0b111_100_111_100_111,
        'F' => 0b111_100_111_100_100,
        'G' => 0b111_100_101_101_111,
        'H' => 0b101_101_111_101_101,
        'I' => 0b111_010_010_010_111,
        'J' => 0b001_001_001_101_111,
        'K' => 0b101_101_110_101_101,
        'L' => 0b100_100_100_100_111,
        'M' => 0b101_111_101_101_101,
        'N' => 0b101_111_101_101_101,
        'O' => 0b111_101_101_101_111,
        'P' => 0b111_101_111_100_100,
        'Q' => 0b111_101_111_011_001,
        'R' => 0b111_101_111_101_101,
        'S' => 0b111_100_111_001_111,
        'T' => 0b111_010_010_010_010,
        'U' => 0b101_101_101_101_111,
        'V' => 0b101_101_101_010_010,
        'W' => 0b101_101_101_111_101,
        'X' => 0b101_101_010_101_101,
        'Y' => 0b101_101_010_010_010,
        'Z' => 0b111_001_010_100_111,
        ':' => 0b000_010_000_010_000,
        '-' => 0b000_000_111_000_000,
        '_' => 0b000_000_000_000_111,
        '.' => 0b000_000_000_000_010,
        '(' => 0b010_100_100_100_010,
        ')' => 0b010_001_001_001_010,
        _ => 0b000_000_000_000_000,
    }
}

pub fn draw_text_with_bg(
    img: &mut RgbaImage,
    x: i32,
    y: i32,
    text: &str,
    fg: Rgba<u8>,
    bg: Rgba<u8>,
    scale: i32,
) {
    let w = text.len() as i32 * 4 * scale;
    let h = 5 * scale;
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px >= 0 && px < img.width() as i32 && py >= 0 && py < img.height() as i32 {
                img.put_pixel(px as u32, py as u32, bg);
            }
        }
    }
    draw_text(img, x, y, text, fg, scale);
}

pub fn draw_char(img: &mut RgbaImage, x: i32, y: i32, c: char, color: Rgba<u8>, scale: i32) {
    let bits = get_char_pixels(c);
    for row in 0..5 {
        for col in 0..3 {
            let bit_idx = 14 - (row * 3 + col);
            let active = ((bits >> bit_idx) & 1) == 1;
            if active {
                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = x + col * scale + dx;
                        let py = y + row * scale + dy;
                        if px >= 0 && px < img.width() as i32 && py >= 0 && py < img.height() as i32
                        {
                            img.put_pixel(px as u32, py as u32, color);
                        }
                    }
                }
            }
        }
    }
}

pub fn draw_text(img: &mut RgbaImage, x: i32, y: i32, text: &str, color: Rgba<u8>, scale: i32) {
    let mut cur_x = x;
    for c in text.chars() {
        draw_char(img, cur_x, y, c, color, scale);
        cur_x += 4 * scale; // 3 columns + 1 space column
    }
}

/// Draw outline bounding box and label for a layer (used in debug_layout mode)
pub fn draw_layer_debug_outline(
    canvas: &mut RgbaImage,
    layer: &ResolvedLayer,
    scene_w: u32,
    scene_h: u32,
    viewport_xmin: f32,
    viewport_ymin: f32,
    disabled_effects: &[String],
) {
    let canvas_w = canvas.width() as f32;
    let canvas_h = canvas.height() as f32;
    let canvas_center = [canvas_w / 2.0, canvas_h / 2.0];

    let layer_w = layer.size[0];
    let layer_h = layer.size[1];

    if layer_w <= 0.0 || layer_h <= 0.0 {
        return;
    }

    // Apply transform-modifying effects
    let (mut location, scale, rotation) = apply_transform_effects(
        &layer.effects,
        layer.location,
        layer.scale,
        layer.rotation,
        layer.time_secs,
        layer.normalized_t,
        disabled_effects,
    );

    // Capture real values before halving for the label
    let rendered_w = layer_w * scale[0].abs();
    let rendered_h = layer_h * scale[1].abs();
    let cx_offset = location[0] - (scene_w as f32 / 2.0);
    let cy_offset = location[1] - (scene_h as f32 / 2.0);

    // Shift locations to match the adaptive viewport
    location[0] = location[0] - viewport_xmin;
    location[1] = location[1] - viewport_ymin;

    // Build forward transform: layer-local -> canvas coordinates
    let fwd = build_transform_matrix(location, scale, rotation, canvas_center);

    let half_w = layer_w / 2.0;
    let half_h = layer_h / 2.0;
    let tl = transform_point(&fwd, [-half_w, -half_h]);
    let tr = transform_point(&fwd, [half_w, -half_h]);
    let br = transform_point(&fwd, [half_w, half_h]);
    let bl = transform_point(&fwd, [-half_w, half_h]);

    let colors = [
        Rgba([0, 255, 0, 255]),
        Rgba([0, 255, 255, 255]),
        Rgba([255, 0, 255, 255]),
        Rgba([255, 255, 0, 255]),
        Rgba([255, 128, 0, 255]),
        Rgba([255, 0, 0, 255]),
        Rgba([128, 0, 255, 255]),
    ];
    let color = colors[(layer.id as usize) % colors.len()];

    // Draw outlines
    draw_line(
        canvas,
        tl[0] as i32,
        tl[1] as i32,
        tr[0] as i32,
        tr[1] as i32,
        color,
    );
    draw_line(
        canvas,
        tr[0] as i32,
        tr[1] as i32,
        br[0] as i32,
        br[1] as i32,
        color,
    );
    draw_line(
        canvas,
        br[0] as i32,
        br[1] as i32,
        bl[0] as i32,
        bl[1] as i32,
        color,
    );
    draw_line(
        canvas,
        bl[0] as i32,
        bl[1] as i32,
        tl[0] as i32,
        tl[1] as i32,
        color,
    );

    // Draw pivot crosshair at the layer's transform origin
    let pivot = transform_point(&fwd, [0.0, 0.0]);
    draw_line(
        canvas,
        pivot[0] as i32 - 6,
        pivot[1] as i32,
        pivot[0] as i32 + 6,
        pivot[1] as i32,
        Rgba([255, 255, 255, 180]),
    );
    draw_line(
        canvas,
        pivot[0] as i32,
        pivot[1] as i32 - 6,
        pivot[0] as i32,
        pivot[1] as i32 + 6,
        Rgba([255, 255, 255, 180]),
    );

    // Draw clean text label (two lines)
    let clean_label: String = layer
        .label
        .as_deref()
        .unwrap_or("Layer")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
        .collect();

    let label_x = tl[0] as i32 + 5;
    let label_y = tl[1] as i32 + 5;
    let bg = Rgba([26, 26, 26, 230]);

    let line1 = format!(
        "{} {}x{} @ {}%  {}x{}",
        clean_label,
        layer_w as u32,
        layer_h as u32,
        (rendered_w / layer_w * 100.0) as u32,
        rendered_w as u32,
        rendered_h as u32
    );
    draw_text_with_bg(canvas, label_x, label_y, &line1, color, bg, 2);

    let line2 = format!(
        "POS {}:{}  ROT {}d",
        cx_offset as i32, cy_offset as i32, rotation as i32
    );
    draw_text_with_bg(
        canvas,
        label_x + 2,
        label_y + 5 * 2 + 2,
        &line2,
        color,
        bg,
        2,
    );
}
