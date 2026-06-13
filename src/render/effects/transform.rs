use crate::model::{Effect, EffectType};

pub fn apply_transform_effects(
    effects: &[Effect],
    location: [f32; 3],
    scale: [f32; 2],
    rotation: f32,
    time_secs: f32,
    normalized_t: f32,
    disabled_effects: &[String],
) -> ([f32; 3], [f32; 2], f32) {
    let mut loc = location;
    let scl = scale;
    let mut rot = rotation;

    for effect in effects {
        if disabled_effects
            .iter()
            .any(|d| d == effect.effect_type.type_name())
        {
            continue;
        }
        match &effect.effect_type {
            EffectType::Oscillate(params) => {
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
                let mag = params.mag.evaluate(normalized_t);
                let evolution = params.evolution.evaluate(normalized_t);
                let seed = params.seed;
                let noise_x = simple_noise(seed, evolution, 0.0);
                let noise_y = simple_noise(seed, evolution, 1.0);
                loc[0] += noise_x * mag;
                loc[1] += noise_y * mag;
            }
            _ => {}
        }
    }

    (loc, scl, rot)
}

fn simple_noise(seed: f32, evolution: f32, offset: f32) -> f32 {
    let v = seed * 12345.6789 + evolution * 6789.12345 + offset * 3456.789;
    (v.sin() * 43758.5453).fract() * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::effect::OscillateParams;
    use crate::model::Animated;

    #[test]
    fn test_oscillate_effect_over_time() {
        let effects = vec![Effect {
            effect_type: EffectType::Oscillate(OscillateParams {
                angle: Animated::Static(0.0),
                freq: Animated::Static(1.0),
                mag: Animated::Static(100.0),
                direction: 0,
                osc_type: 0,
                phase: 0.0,
            }),
            locally_applied: true,
        }];

        let location = [0.0, 0.0, 0.0];
        let scale = [1.0, 1.0];
        let rotation = 0.0;

        let disabled = &[];

        let (loc0, _, _) =
            apply_transform_effects(&effects, location, scale, rotation, 0.0, 0.0, disabled);
        let (loc_quarter, _, _) =
            apply_transform_effects(&effects, location, scale, rotation, 0.25, 0.25, disabled);
        let (loc_half, _, _) =
            apply_transform_effects(&effects, location, scale, rotation, 0.5, 0.5, disabled);

        assert_eq!(loc0[0], 0.0);
        assert!((loc_quarter[0] - 100.0).abs() < 1e-4);
        assert!((loc_half[0] - 0.0).abs() < 1e-4);
    }
}
