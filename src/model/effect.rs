// model/effect.rs — Effect types and parameter definitions

use super::animation::Animated;

// ---------------------------------------------------------------------------
// Top-level effect types
// ---------------------------------------------------------------------------

/// Classification of visual effects by their behavior.
///
/// Each variant wraps a parameter struct containing the effect's animatable
/// and static properties. Unknown effects preserve their raw ID string for
/// diagnostic purposes.
#[derive(Debug, Clone)]
pub enum EffectType {
    // -- Transform modifiers --
    /// Periodic oscillation of position/scale/rotation.
    Oscillate(OscillateParams),
    /// Pendulum-style rotation swing.
    Swing(SwingParams),
    /// Randomized displacement per frame.
    RandomDisplace(RandomDisplaceParams),

    // -- Temporal --
    /// Simulated motion blur based on layer velocity.
    MotionBlur(MotionBlurParams),
    /// Periodic on/off blinking.
    Blink(BlinkParams),
    /// Fade in/out at layer boundaries.
    Fade(FadeParams),

    // -- UV --
    /// Tiled repetition of the layer content.
    Tile(TileParams),

    // -- Color / compositing --
    /// Exposure and gamma adjustment.
    Exposure(ExposureParams),
    /// Brightness and contrast adjustment.
    BrightnessContrast(BrightnessContrastParams),
    /// Saturation and vibrance adjustment.
    SaturationVibrance(SaturationVibranceParams),
    /// Solid color tint overlay.
    ColorTint(ColorTintParams),
    /// Highlight and shadow recovery.
    HighlightShadow(HighlightShadowParams),
    /// Vignette darkening around edges.
    Vignette(VignetteParams),
    /// Unsharp mask sharpening.
    Sharpen(SharpenParams),
    /// Gaussian (box-approximated) blur.
    GaussianBlur(GaussianBlurParams),
    /// Lens/bokeh blur simulation.
    LensBlur(LensBlurParams),
    /// Linear/radial gradient overlay.
    GradientOverlay(GradientOverlayParams),
    /// Lift adjustment (shadows color shift). No parameters.
    Lift,

    // -- Keying --
    /// Luminance-based alpha keying.
    LumaKey(LumaKeyParams),

    /// Unknown or unsupported effect — stores the raw effect ID from XML.
    Unknown(String),
}

/// A fully resolved effect attached to a layer.
#[derive(Debug, Clone)]
pub struct Effect {
    /// The effect variant and its parameters.
    pub effect_type: EffectType,
    /// Whether this effect is locally applied (per-layer) rather than
    /// inherited from a parent group.
    pub locally_applied: bool,
}

// ---------------------------------------------------------------------------
// Parameter structs
// ---------------------------------------------------------------------------

/// Parameters for the oscillation effect.
#[derive(Debug, Clone)]
pub struct OscillateParams {
    /// Oscillation angle in degrees.
    pub angle: Animated<f32>,
    /// Oscillation frequency in Hz.
    pub freq: Animated<f32>,
    /// Oscillation magnitude in pixels.
    pub mag: Animated<f32>,
    /// Oscillation direction (0 = x, 1 = y, 2 = both, etc.).
    pub direction: i32,
    /// Oscillation waveform type (0 = sine, 1 = triangle, etc.).
    pub osc_type: i32,
    /// Initial phase offset in radians.
    pub phase: f32,
}

impl Default for OscillateParams {
    fn default() -> Self {
        Self {
            angle: Animated::Static(0.0),
            freq: Animated::Static(1.0),
            mag: Animated::Static(10.0),
            direction: 0,
            osc_type: 0,
            phase: 0.0,
        }
    }
}

/// Parameters for the swing (pendulum) effect.
#[derive(Debug, Clone)]
pub struct SwingParams {
    /// Primary amplitude in degrees.
    pub a1: Animated<f32>,
    /// Secondary amplitude in degrees.
    pub a2: Animated<f32>,
    /// Swing frequency in Hz.
    pub freq: Animated<f32>,
}

impl Default for SwingParams {
    fn default() -> Self {
        Self {
            a1: Animated::Static(15.0),
            a2: Animated::Static(0.0),
            freq: Animated::Static(1.0),
        }
    }
}

/// Parameters for random displacement.
#[derive(Debug, Clone)]
pub struct RandomDisplaceParams {
    /// Evolution speed (animated noise offset).
    pub evolution: Animated<f32>,
    /// Displacement magnitude in pixels.
    pub mag: Animated<f32>,
    /// Random seed.
    pub seed: f32,
    /// Scatter factor controlling displacement distribution.
    pub scatter: f32,
}

impl Default for RandomDisplaceParams {
    fn default() -> Self {
        Self {
            evolution: Animated::Static(0.0),
            mag: Animated::Static(10.0),
            seed: 0.0,
            scatter: 1.0,
        }
    }
}

/// Parameters for simulated motion blur.
#[derive(Debug, Clone)]
pub struct MotionBlurParams {
    /// Blur intensity / shutter angle tuning.
    pub tune: Animated<f32>,
    /// Whether to blur based on position changes.
    pub use_pos: bool,
    /// Whether to blur based on scale changes.
    pub use_scale: bool,
    /// Whether to blur based on rotation changes.
    pub use_angle: bool,
}

impl Default for MotionBlurParams {
    fn default() -> Self {
        Self {
            tune: Animated::Static(1.0),
            use_pos: true,
            use_scale: true,
            use_angle: true,
        }
    }
}

/// Parameters for periodic blinking.
#[derive(Debug, Clone)]
pub struct BlinkParams {
    /// Blink frequency in Hz.
    pub freq: Animated<f32>,
}

impl Default for BlinkParams {
    fn default() -> Self {
        Self {
            freq: Animated::Static(2.0),
        }
    }
}

/// Parameters for fade in/out.
#[derive(Debug, Clone)]
pub struct FadeParams {
    /// Fade-in duration in milliseconds.
    pub in_time: f32,
    /// Fade-out duration in milliseconds.
    pub out_time: f32,
}

impl Default for FadeParams {
    fn default() -> Self {
        Self {
            in_time: 200.0,
            out_time: 200.0,
        }
    }
}

/// Parameters for tiled repetition.
#[derive(Debug, Clone)]
pub struct TileParams {
    /// Whether tiles are mirrored at boundaries.
    pub mirror: bool,
    /// Tile scale factor.
    pub scale: f32,
    /// Tile phase offset.
    pub phase: f32,
    /// Whether vertical offset alternation is applied.
    pub vert_offset: bool,
    /// Tile rotation angle in degrees.
    pub angle: f32,
}

impl Default for TileParams {
    fn default() -> Self {
        Self {
            mirror: false,
            scale: 1.0,
            phase: 0.0,
            vert_offset: false,
            angle: 0.0,
        }
    }
}

/// Parameters for exposure adjustment.
#[derive(Debug, Clone)]
pub struct ExposureParams {
    /// Exposure value in stops.
    pub exposure: Animated<f32>,
    /// Gamma correction factor.
    pub gamma: Animated<f32>,
    /// Offset added after exposure/gamma.
    pub offset: f32,
}

impl Default for ExposureParams {
    fn default() -> Self {
        Self {
            exposure: Animated::Static(0.0),
            gamma: Animated::Static(1.0),
            offset: 0.0,
        }
    }
}

/// Parameters for brightness and contrast adjustment.
#[derive(Debug, Clone)]
pub struct BrightnessContrastParams {
    /// Brightness offset (-1.0 to 1.0).
    pub brightness: f32,
    /// Contrast multiplier (-1.0 to 1.0).
    pub contrast: f32,
}

impl Default for BrightnessContrastParams {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 0.0,
        }
    }
}

/// Parameters for saturation and vibrance adjustment.
#[derive(Debug, Clone)]
pub struct SaturationVibranceParams {
    /// Saturation multiplier (-1.0 to 1.0).
    pub saturation: f32,
    /// Vibrance (intelligent saturation) adjustment (-1.0 to 1.0).
    pub vibrance: f32,
}

impl Default for SaturationVibranceParams {
    fn default() -> Self {
        Self {
            saturation: 0.0,
            vibrance: 0.0,
        }
    }
}

/// Parameters for color tint.
#[derive(Debug, Clone)]
pub struct ColorTintParams {
    /// RGB tint color (0.0–1.0 per channel).
    pub tint: [f32; 3],
}

impl Default for ColorTintParams {
    fn default() -> Self {
        Self {
            tint: [1.0, 1.0, 1.0],
        }
    }
}

/// Parameters for highlight and shadow adjustment.
#[derive(Debug, Clone)]
pub struct HighlightShadowParams {
    /// Highlight recovery amount (-1.0 to 1.0).
    pub highlights: f32,
    /// Shadow recovery amount (-1.0 to 1.0).
    pub shadows: f32,
}

impl Default for HighlightShadowParams {
    fn default() -> Self {
        Self {
            highlights: 0.0,
            shadows: 0.0,
        }
    }
}

/// Parameters for vignette effect.
#[derive(Debug, Clone)]
pub struct VignetteParams {
    /// Feather (softness) of the vignette edge.
    pub feather: f32,
    /// Roundness of the vignette shape (0.0 = rectangular, 1.0 = circular).
    pub roundness: f32,
    /// Scale of the vignette area.
    pub scale: f32,
    /// Darkening strength (0.0–1.0).
    pub strength: f32,
    /// Tint amount applied to vignetted regions.
    pub tint: f32,
}

impl Default for VignetteParams {
    fn default() -> Self {
        Self {
            feather: 0.5,
            roundness: 1.0,
            scale: 1.0,
            strength: 0.5,
            tint: 0.0,
        }
    }
}

/// Parameters for unsharp mask sharpening.
#[derive(Debug, Clone)]
pub struct SharpenParams {
    /// Sharpening radius in pixels.
    pub radius: f32,
    /// Sharpening strength (0.0–1.0+).
    pub strength: f32,
}

impl Default for SharpenParams {
    fn default() -> Self {
        Self {
            radius: 1.0,
            strength: 0.5,
        }
    }
}

/// Parameters for Gaussian blur.
#[derive(Debug, Clone)]
pub struct GaussianBlurParams {
    /// Blur radius in pixels.
    pub radius: f32,
}

impl Default for GaussianBlurParams {
    fn default() -> Self {
        Self { radius: 5.0 }
    }
}

/// Parameters for lens (bokeh) blur.
#[derive(Debug, Clone)]
pub struct LensBlurParams {
    /// Blur radius in pixels.
    pub radius: f32,
    /// Blur strength / quality factor.
    pub strength: f32,
}

impl Default for LensBlurParams {
    fn default() -> Self {
        Self {
            radius: 5.0,
            strength: 1.0,
        }
    }
}

/// Parameters for gradient overlay.
#[derive(Debug, Clone)]
pub struct GradientOverlayParams {
    /// Overall alpha of the gradient overlay (0.0–1.0).
    pub alpha: f32,
    /// Start color (RGBA, 0.0–1.0).
    pub color1: [f32; 4],
    /// End color (RGBA, 0.0–1.0).
    pub color2: [f32; 4],
    /// Gradient center offset in normalized coordinates.
    pub offset: [f32; 2],
    /// Gradient scale factor.
    pub scale: f32,
}

impl Default for GradientOverlayParams {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            color1: [0.0, 0.0, 0.0, 1.0],
            color2: [1.0, 1.0, 1.0, 1.0],
            offset: [0.0, 0.0],
            scale: 1.0,
        }
    }
}

/// Parameters for luminance-based alpha keying.
#[derive(Debug, Clone)]
pub struct LumaKeyParams {
    /// Lower luminance threshold (pixels below this become transparent).
    pub low_threshold: Animated<f32>,
    /// Upper luminance threshold (pixels above this become transparent).
    pub high_threshold: Animated<f32>,
}

impl Default for LumaKeyParams {
    fn default() -> Self {
        Self {
            low_threshold: Animated::Static(0.0),
            high_threshold: Animated::Static(1.0),
        }
    }
}
