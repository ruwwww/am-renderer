pub fn apply_fade(opacity: f32, t: f32, duration_ms: f32, in_time: f32, out_time: f32) -> f32 {
    let mut opacity = opacity;
    let elapsed_ms = t * duration_ms;

    if in_time > 0.0 && elapsed_ms < in_time {
        let factor = (elapsed_ms / in_time).clamp(0.0, 1.0);
        opacity *= factor;
    }

    let remaining_ms = duration_ms - elapsed_ms;
    if out_time > 0.0 && remaining_ms < out_time {
        let factor = (remaining_ms / out_time).clamp(0.0, 1.0);
        opacity *= factor;
    }

    opacity
}
