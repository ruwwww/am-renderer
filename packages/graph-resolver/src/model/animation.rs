//! Animation model types — `Animated<T>`, `Keyframe`, easing, and interpolation.

use serde::{Serialize, Deserialize};
use glam::Vec2;

/// Easing type for keyframe interpolation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EasingType {
    /// Linear interpolation (no easing).
    Linear,
    /// Cubic Bézier easing with control points (x1, y1, x2, y2).
    CubicBezier(f32, f32, f32, f32),
}

impl Default for EasingType {
    fn default() -> Self {
        Self::Linear
    }
}

/// A single keyframe in an animation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyframe<T: Clone> {
    /// Normalised time within the layer (0.0 = start, 1.0 = end).
    pub t: f32,
    /// Value at this keyframe.
    pub value: T,
    /// Easing applied *from this keyframe to the next*.
    pub easing: EasingType,
}

/// Trait for values that can be linearly interpolated.
pub trait Lerp {
    /// Linearly interpolate between `self` and `other` by factor `t`.
    fn lerp(&self, other: &Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self + (other - self) * t
    }
}

impl Lerp for [f32; 2] {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        [
            self[0] + (other[0] - self[0]) * t,
            self[1] + (other[1] - self[1]) * t,
        ]
    }
}

impl Lerp for [f32; 3] {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        [
            self[0] + (other[0] - self[0]) * t,
            self[1] + (other[1] - self[1]) * t,
            self[2] + (other[2] - self[2]) * t,
        ]
    }
}

impl Lerp for Vec2 {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        glam::Vec2::lerp(*self, *other, t)
    }
}

/// An animatable property — either a static value or a list of keyframes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Animated<T: Clone> {
    /// Constant value across all time.
    Static(T),
    /// Animated via keyframes.
    Keyframed(Vec<Keyframe<T>>),
}

impl<T: Clone + Lerp> Animated<T> {
    /// Evaluate this property at normalised time `t` (0.0–1.0).
    pub fn evaluate(&self, t: f32) -> T {
        match self {
            Animated::Static(v) => v.clone(),
            Animated::Keyframed(kfs) => {
                if kfs.is_empty() {
                    panic!("Animated::Keyframed with no keyframes");
                }
                if kfs.len() == 1 || t <= kfs[0].t {
                    return kfs[0].value.clone();
                }
                if t >= kfs.last().unwrap().t {
                    return kfs.last().unwrap().value.clone();
                }
                // Find the surrounding keyframes
                for i in 0..kfs.len() - 1 {
                    let k0 = &kfs[i];
                    let k1 = &kfs[i + 1];
                    if t >= k0.t && t <= k1.t {
                        let seg_t = if (k1.t - k0.t).abs() < 1e-10 {
                            0.0
                        } else {
                            (t - k0.t) / (k1.t - k0.t)
                        };
                        let eased_t = apply_easing(seg_t, &k0.easing);
                        return k0.value.lerp(&k1.value, eased_t);
                    }
                }
                kfs.last().unwrap().value.clone()
            }
        }
    }
}

/// Apply easing to a linear t value.
fn apply_easing(t: f32, easing: &EasingType) -> f32 {
    match easing {
        EasingType::Linear => t,
        EasingType::CubicBezier(x1, y1, x2, y2) => cubic_bezier_sample(t, *x1, *y1, *x2, *y2),
    }
}

/// Sample a cubic Bézier easing curve at progress `t`.
///
/// Control points are (0,0), (x1,y1), (x2,y2), (1,1).
/// Uses Newton's method to solve for the parametric t from the x coordinate.
fn cubic_bezier_sample(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    // Find parametric u such that bezier_x(u) == t
    let mut u = t;
    for _ in 0..8 {
        let x = bezier_component(u, x1, x2) - t;
        let dx = bezier_component_deriv(u, x1, x2);
        if dx.abs() < 1e-10 {
            break;
        }
        u -= x / dx;
        u = u.clamp(0.0, 1.0);
    }
    bezier_component(u, y1, y2)
}

fn bezier_component(t: f32, p1: f32, p2: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    3.0 * (1.0 - t) * (1.0 - t) * t * p1 + 3.0 * (1.0 - t) * t2 * p2 + t3
}

fn bezier_component_deriv(t: f32, p1: f32, p2: f32) -> f32 {
    let t2 = t * t;
    3.0 * (1.0 - t) * (1.0 - t) * p1 + 6.0 * (1.0 - t) * t * (p2 - p1) + 3.0 * t2 * (1.0 - p2)
}
