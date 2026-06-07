//! Effect evaluation — applies transform-modifying effects to resolved layer properties.
//!
//! Transform-modifying effects (Oscillate, Swing, RandomDisplace) modify
//! the layer's position, scale, or rotation before compositing. Other effects
//! (blur, color adjustments) are applied at render time to the pixel buffer.

use crate::model::{EffectType, Effect};

/// Apply transform-modifying effects to a resolved layer's transform.
///
/// Iterates over the layer's effects and modifies position, scale, and
/// rotation for effects that operate in transform space (Oscillate, Swing,
/// RandomDisplace).
///
/// # Arguments
/// * `effects` - Slice of effects to apply
/// * `location` - Current layer position [x, y, z]
/// * `scale` - Current layer scale [sx, sy]
/// * `rotation` - Current rotation in degrees
/// * `time_secs` - Current time in seconds (for periodic effects)
/// * `normalized_t` - Normalized time within the layer [0, 1]
///
/// # Returns
/// Modified (location, scale, rotation) tuple.
pub fn apply_transform_effects(
    effects: &[Effect],
    location: [f32; 3],
    scale: [f32; 2],
    rotation: f32,
    time_secs: f32,
    normalized_t: f32,
) -> ([f32; 3], [f32; 2], f32) {
    let mut loc = location;
    let scl = scale;
    let mut rot = rotation;

    for effect in effects {
        match &effect.effect_type {
            EffectType::Oscillate(params) => {
                // Periodic displacement along an angle
                let freq = params.freq.evaluate(normalized_t);
                let mag = params.mag.evaluate(normalized_t);
                let angle = params.angle.evaluate(normalized_t);
                let angle_rad = angle.to_radians();
                let phase = params.phase;
                let osc = (time_secs * freq * std::f32::consts::TAU + phase).sin() * mag;
                loc[0] += osc * angle_rad.cos();
                loc[1] += osc * angle_rad.sin();
            }
            EffectType::Swing(params) => {
                // Periodic rotation between two angle limits
                let a1 = params.a1.evaluate(normalized_t);
                let a2 = params.a2.evaluate(normalized_t);
                let freq = params.freq.evaluate(normalized_t);
                let swing = (time_secs * freq * std::f32::consts::TAU).sin();
                let swing_angle = if swing >= 0.0 {
                    swing * a2
                } else {
                    swing * (-a1)
                };
                rot += swing_angle;
            }
            EffectType::RandomDisplace(params) => {
                // Deterministic noise-based displacement
                let mag = params.mag.evaluate(normalized_t);
                let evolution = params.evolution.evaluate(normalized_t);
                let seed = params.seed;
                let noise_x = simple_noise(seed, evolution, 0.0);
                let noise_y = simple_noise(seed, evolution, 1.0);
                loc[0] += noise_x * mag;
                loc[1] += noise_y * mag;
            }
            _ => {
                // Other effects (blur, color, etc.) are handled at render time
            }
        }
    }

    (loc, scl, rot)
}

/// Simple deterministic noise function.
///
/// Produces a pseudo-random value in [-1, 1] based on seed, evolution, and offset.
/// Deterministic for the same inputs — not cryptographically random.
fn simple_noise(seed: f32, evolution: f32, offset: f32) -> f32 {
    let v = seed * 12345.6789 + evolution * 6789.12345 + offset * 3456.789;
    (v.sin() * 43758.5453).fract() * 2.0 - 1.0
}
