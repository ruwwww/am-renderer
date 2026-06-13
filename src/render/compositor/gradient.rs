use super::utils::to_rgba_u8;
use crate::model::{Gradient, GradientStop};
use image::RgbaImage;

/// Render a gradient into an image buffer.
pub(crate) fn render_gradient(w: u32, h: u32, gradient: &Gradient) -> RgbaImage {
    let mut img = RgbaImage::new(w, h);

    if gradient.stops.is_empty() {
        return img;
    }

    let start = gradient.start;
    let end = gradient.end;
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let len_sq = dx * dx + dy * dy;

    for y in 0..h {
        for x in 0..w {
            // Normalize coordinates to [0, 1]
            let nx = x as f32 / w.max(1) as f32;
            let ny = y as f32 / h.max(1) as f32;

            // Compute linear gradient position by projecting onto the segment start -> end
            let t = if len_sq > 1e-6 {
                let px = nx - start[0];
                let py = ny - start[1];
                ((px * dx + py * dy) / len_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let color = sample_gradient(&gradient.stops, t);
            img.put_pixel(x, y, to_rgba_u8(color));
        }
    }

    img
}

/// Sample a gradient at position t, interpolating between stops.
fn sample_gradient(stops: &[GradientStop], t: f32) -> [f32; 4] {
    if stops.is_empty() {
        return [0.0, 0.0, 0.0, 0.0];
    }
    if stops.len() == 1 {
        return stops[0].color;
    }

    let t = t.clamp(0.0, 1.0);

    // Before first stop
    if t <= stops[0].position {
        return stops[0].color;
    }
    // After last stop
    if t >= stops[stops.len() - 1].position {
        return stops[stops.len() - 1].color;
    }

    // Find surrounding stops
    for i in 0..stops.len() - 1 {
        if t >= stops[i].position && t <= stops[i + 1].position {
            let range = stops[i + 1].position - stops[i].position;
            let local_t = if range > f32::EPSILON {
                (t - stops[i].position) / range
            } else {
                0.0
            };
            return [
                stops[i].color[0] + (stops[i + 1].color[0] - stops[i].color[0]) * local_t,
                stops[i].color[1] + (stops[i + 1].color[1] - stops[i].color[1]) * local_t,
                stops[i].color[2] + (stops[i + 1].color[2] - stops[i].color[2]) * local_t,
                stops[i].color[3] + (stops[i + 1].color[3] - stops[i].color[3]) * local_t,
            ];
        }
    }

    stops[stops.len() - 1].color
}
