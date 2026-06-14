//! Layer model types.

use serde::{Serialize, Deserialize};
use super::animation::Animated;
use super::effect::Effect;

/// Blend mode for layer compositing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    /// Normal alpha compositing.
    Normal,
    /// Multiply blend.
    Multiply,
    /// Screen blend.
    Screen,
    /// Overlay blend.
    Overlay,
    /// Darken blend.
    Darken,
    /// Lighten blend.
    Lighten,
    /// Subtract blend.
    Subtract,
    /// Additive blend.
    Add,
}

impl Default for BlendMode {
    fn default() -> Self {
        Self::Normal
    }
}

/// Fill type for a shape layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillType {
    /// No fill / transparent.
    None,
    /// Filled with a media image.
    Media,
    /// Filled with a solid color.
    Color,
    /// Filled with a gradient.
    Gradient,
}

impl Default for FillType {
    fn default() -> Self {
        Self::None
    }
}

/// A gradient stop for gradient fills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradientStop {
    /// Position along the gradient (0.0–1.0).
    pub position: f32,
    /// RGBA color [0.0–1.0].
    pub color: [f32; 4],
}

/// A gradient fill specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gradient {
    /// Start position (normalised coordinates).
    pub start: [f32; 2],
    /// End position (normalised coordinates).
    pub end: [f32; 2],
    /// Gradient color stops.
    pub stops: Vec<GradientStop>,
}

/// Animated transform properties for a layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerTransform {
    /// Translation in scene coordinates [x, y, z].
    pub location: Animated<[f32; 3]>,
    /// Scale factor [sx, sy].
    pub scale: Animated<[f32; 2]>,
    /// Rotation in degrees.
    pub rotation: Animated<f32>,
    /// Opacity (0.0–1.0).
    pub opacity: Animated<f32>,
}

/// A visual layer (shape) in the project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    /// Unique layer id.
    pub id: u64,
    /// User-visible label.
    pub label: Option<String>,
    /// Start time in milliseconds.
    pub start_time: f32,
    /// End time in milliseconds.
    pub end_time: f32,
    /// Whether this layer is hidden.
    pub hidden: bool,
    /// Transform (location / scale / rotation / opacity).
    pub transform: LayerTransform,
    /// Fill type.
    pub fill_type: FillType,
    /// URI of the fill image (when fill_type == Media).
    pub fill_image: Option<String>,
    /// Fill color as RGBA [0.0–1.0].
    pub fill_color: [f32; 4],
    /// Optional gradient fill.
    pub gradient: Option<Gradient>,
    /// Blend mode.
    pub blend_mode: BlendMode,
    /// How the media fills the shape (e.g. "stretch", "fit", "fill").
    pub media_fill_mode: Option<String>,
    /// Applied effects.
    pub effects: Vec<Effect>,
    /// Layer size [w, h] in pixels.
    pub size: [f32; 2],
    /// Shape primitive name (e.g. ".rect", ".circle", ".roundrect").
    pub s: Option<String>,
}

/// A resolved (evaluated at a specific time) layer with concrete values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedLayer {
    /// Layer id.
    pub id: u64,
    /// User-visible label.
    pub label: Option<String>,
    /// Resolved position [x, y, z].
    pub location: [f32; 3],
    /// Resolved scale [sx, sy].
    pub scale: [f32; 2],
    /// Resolved rotation in degrees.
    pub rotation: f32,
    /// Resolved opacity (0.0–1.0).
    pub opacity: f32,
    /// Fill type.
    pub fill_type: FillType,
    /// Fill image URI.
    pub fill_image: Option<String>,
    /// Fill color RGBA.
    pub fill_color: [f32; 4],
    /// Gradient fill.
    pub gradient: Option<Gradient>,
    /// Blend mode.
    pub blend_mode: BlendMode,
    /// How the media fills the shape (e.g. "stretch", "fit", "fill").
    pub media_fill_mode: Option<String>,
    /// Effects (already evaluated params where applicable).
    pub effects: Vec<Effect>,
    /// Layer size [w, h] in pixels.
    pub size: [f32; 2],
    /// Shape primitive name (e.g. ".rect", ".circle", ".roundrect").
    pub s: Option<String>,
    /// Evaluation time in seconds.
    pub time_secs: f32,
    /// Normalized time [0, 1] relative to the layer duration.
    pub normalized_t: f32,
}
