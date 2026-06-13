//! Timeline evaluator — resolves a project at a specific point in time.
//!
//! Given a [`Project`] and a time in seconds, this module produces a
//! [`ResolvedScene`] containing all visible layers with their animated
//! properties evaluated to concrete values.

use crate::model::{Layer, Project, ResolvedLayer};

/// A fully resolved scene at a specific point in time.
///
/// Contains the canvas dimensions, background color, and all visible
/// layers with their properties evaluated.
#[derive(Debug, Clone)]
pub struct ResolvedScene {
    /// Canvas width in pixels
    pub width: u32,
    /// Canvas height in pixels
    pub height: u32,
    /// Background color as RGBA [0.0 - 1.0]
    pub bg_color: [f32; 4],
    /// Resolved layers, ordered bottom to top for compositing
    pub layers: Vec<ResolvedLayer>,
}

/// Evaluate the project at a given time in seconds.
///
/// Iterates over all layers, skipping hidden or out-of-range layers,
/// and evaluates animated properties at the appropriate normalized time.
///
/// # Arguments
/// * `project` - The project to evaluate
/// * `time_secs` - Time in seconds
///
/// # Returns
/// A [`ResolvedScene`] with all visible layers resolved.
pub fn evaluate(project: &Project, time_secs: f32) -> ResolvedScene {
    let time_ms = time_secs * 1000.0;
    let mut resolved_layers = Vec::new();

    for layer in &project.layers {
        // Skip hidden layers
        if layer.hidden {
            continue;
        }

        // Skip layers outside their time range
        if time_ms < layer.start_time || time_ms >= layer.end_time {
            continue;
        }

        // Calculate normalized time [0, 1] within this layer's duration
        let duration = layer.end_time - layer.start_time;
        let normalized_t = if duration > 0.0 {
            (time_ms - layer.start_time) / duration
        } else {
            0.0
        };

        let resolved = resolve_layer(layer, normalized_t, time_secs);
        resolved_layers.push(resolved);
    }

    ResolvedScene {
        width: project.width,
        height: project.height,
        bg_color: project.bg_color,
        layers: resolved_layers,
    }
}

fn integrate_spin_numeric(
    rpm_animated: &crate::model::Animated<f32>,
    t: f32,
    duration_secs: f32,
) -> f32 {
    match rpm_animated {
        crate::model::Animated::Static(rpm) => rpm * t * duration_secs * 6.0,
        crate::model::Animated::Keyframed(_) => {
            let n = 100;
            let step = t / n as f32;
            let mut sum = 0.0;
            let mut prev_val = rpm_animated.evaluate(0.0);
            for i in 1..=n {
                let u = i as f32 * step;
                let val = rpm_animated.evaluate(u);
                sum += (prev_val + val) * 0.5 * step;
                prev_val = val;
            }
            sum * duration_secs * 6.0
        }
    }
}

/// Resolve a single layer at a given normalized time.
///
/// Evaluates all animated transform properties (location, scale, rotation,
/// opacity) and copies static properties through to the resolved layer.
fn resolve_layer(layer: &Layer, t: f32, time_secs: f32) -> ResolvedLayer {
    let mut opacity = layer.transform.opacity.evaluate(t);

    // Apply fade effect if present
    for effect in &layer.effects {
        if let crate::model::EffectType::Fade(ref params) = effect.effect_type {
            let duration_ms = layer.end_time - layer.start_time;
            opacity = crate::render::effects::fade::apply_fade(
                opacity,
                t,
                duration_ms,
                params.in_time,
                params.out_time,
            );
        }
    }

    let mut rotation = layer.transform.rotation.evaluate(t);

    // Apply spin effect if present
    for effect in &layer.effects {
        if let crate::model::EffectType::Spin(ref params) = effect.effect_type {
            let duration_ms = layer.end_time - layer.start_time;
            let duration_secs = duration_ms / 1000.0;
            rotation += integrate_spin_numeric(&params.rpm, t, duration_secs);
        }
    }

    ResolvedLayer {
        id: layer.id,
        label: layer.label.clone(),
        location: layer.transform.location.evaluate(t),
        scale: layer.transform.scale.evaluate(t),
        rotation,
        opacity,
        fill_type: layer.fill_type,
        fill_image: layer.fill_image.clone(),
        fill_color: layer.fill_color,
        gradient: layer.gradient.clone(),
        blend_mode: layer.blend_mode,
        media_fill_mode: layer.media_fill_mode.clone(),
        effects: layer.effects.clone(),
        size: layer.size,
        s: layer.s.clone(),
        time_secs,
        normalized_t: t,
    }
}
