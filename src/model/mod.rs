//! Domain model types for the am-renderer project.
//!
//! This module contains all the core data structures used throughout
//! the renderer pipeline: project definitions, layers, animations,
//! effects, and resolved (evaluated) layer states.

pub mod animation;
pub mod effect;
pub mod keyframe;
pub mod layer;
pub mod project;

// Re-export commonly used types for convenience
pub use animation::{Animated, EasingType, Keyframe, Lerp};
pub use effect::{Effect, EffectType};
pub use layer::{
    BlendMode, FillType, Gradient, GradientStop, Layer, LayerTransform, ResolvedLayer,
};
pub use project::{AudioTrack, MediaRef, Project};
