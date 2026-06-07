//! Domain model types for the am-renderer project.
//!
//! This module contains all the core data structures used throughout
//! the renderer pipeline: project definitions, layers, animations,
//! effects, and resolved (evaluated) layer states.

pub mod project;
pub mod layer;
pub mod animation;
pub mod keyframe;
pub mod effect;

// Re-export commonly used types for convenience
pub use project::{Project, MediaRef, AudioTrack};
pub use layer::{Layer, ResolvedLayer, LayerTransform, BlendMode, FillType, Gradient, GradientStop};
pub use animation::{Animated, Keyframe, EasingType, Lerp};
pub use effect::{Effect, EffectType};
