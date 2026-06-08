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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Animated;
    use crate::model::effect::OscillateParams;

    #[test]
    fn test_oscillate_effect_over_time() {
        let effects = vec![Effect {
            effect_type: EffectType::Oscillate(OscillateParams {
                angle: Animated::Static(0.0), // along x-axis (cos(0) = 1, sin(0) = 0)
                freq: Animated::Static(1.0),  // 1 Hz
                mag: Animated::Static(100.0), // 100 pixels magnitude
                direction: 0,
                osc_type: 0,
                phase: 0.0,
            }),
            locally_applied: true,
        }];

        let location = [0.0, 0.0, 0.0];
        let scale = [1.0, 1.0];
        let rotation = 0.0;

        // Evaluate at different times
        let (loc0, _, _) = apply_transform_effects(&effects, location, scale, rotation, 0.0, 0.0);
        let (loc_quarter, _, _) = apply_transform_effects(&effects, location, scale, rotation, 0.25, 0.25);
        let (loc_half, _, _) = apply_transform_effects(&effects, location, scale, rotation, 0.5, 0.5);

        // Sin wave with phase 0:
        // at t = 0.0: sin(0) = 0 -> displacement = 0
        // at t = 0.25: sin(0.25 * 1 * 2pi) = sin(pi/2) = 1 -> displacement = 100
        // at t = 0.5: sin(0.5 * 1 * 2pi) = sin(pi) = 0 -> displacement = 0
        assert_eq!(loc0[0], 0.0);
        assert!((loc_quarter[0] - 100.0).abs() < 1e-4);
        assert!((loc_half[0] - 0.0).abs() < 1e-4);
    }
}
