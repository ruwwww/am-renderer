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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// Lift adjustment (shadows color shift / copy background).
    Lift(LiftParams),
    /// Colorize effect (shift hue preserving luminance).
    Colorize(ColorizeParams),

    // -- Keying --
    /// Luminance-based alpha keying.
    LumaKey(LumaKeyParams),

    // -- Distort / Transform --
    /// Horizontal/vertical shift (offset).
    Offset(OffsetParams),
    /// Stylized edge outlines detection.
    FindEdges(FindEdgesParams),
    /// Segment stretching distortion.
    StretchSegment(StretchSegmentParams),
    /// Spin auto-rotation effect.
    Spin(SpinParams),
    /// Swirl warp effect.
    Swirl(SwirlParams),
    /// Wipe transition/mask effect.
    Wipe(WipeParams),

    /// Unknown or unsupported effect — stores the raw effect ID from XML.
    Unknown(String),
}

impl EffectType {
    /// Return the variant name as a static string, for use with config-based
    /// effect filtering.
    pub fn type_name(&self) -> &'static str {
        match self {
            EffectType::Oscillate(_) => "Oscillate",
            EffectType::Swing(_) => "Swing",
            EffectType::RandomDisplace(_) => "RandomDisplace",
            EffectType::MotionBlur(_) => "MotionBlur",
            EffectType::Blink(_) => "Blink",
            EffectType::Fade(_) => "Fade",
            EffectType::Tile(_) => "Tile",
            EffectType::Exposure(_) => "Exposure",
            EffectType::BrightnessContrast(_) => "BrightnessContrast",
            EffectType::SaturationVibrance(_) => "SaturationVibrance",
            EffectType::ColorTint(_) => "ColorTint",
            EffectType::HighlightShadow(_) => "HighlightShadow",
            EffectType::Vignette(_) => "Vignette",
            EffectType::Sharpen(_) => "Sharpen",
            EffectType::GaussianBlur(_) => "GaussianBlur",
            EffectType::LensBlur(_) => "LensBlur",
            EffectType::GradientOverlay(_) => "GradientOverlay",
            EffectType::Lift(_) => "Lift",
            EffectType::Colorize(_) => "Colorize",
            EffectType::LumaKey(_) => "LumaKey",
            EffectType::Offset(_) => "Offset",
            EffectType::FindEdges(_) => "FindEdges",
            EffectType::StretchSegment(_) => "StretchSegment",
            EffectType::Spin(_) => "Spin",
            EffectType::Swirl(_) => "Swirl",
            EffectType::Wipe(_) => "Wipe",
            EffectType::Unknown(_) => "Unknown",
        }
    }
}

/// A fully resolved effect attached to a layer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TileParams {
    /// Whether tiles are mirrored at boundaries.
    pub mirror: bool,
    /// Tile scale factor.
    pub scale: Animated<f32>,
    /// Tile phase offset.
    pub phase: Animated<f32>,
    /// Whether vertical offset alternation is applied.
    pub vert_offset: bool,
    /// Tile rotation angle in degrees.
    pub angle: Animated<f32>,
}

impl Default for TileParams {
    fn default() -> Self {
        Self {
            mirror: false,
            scale: Animated::Static(1.0),
            phase: Animated::Static(0.0),
            vert_offset: false,
            angle: Animated::Static(0.0),
        }
    }
}

/// Parameters for exposure adjustment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// Color of the vignette (overlay color).
    pub overlaycolor: [f32; 4],
    /// Punchout mode (makes center transparent instead of color blending).
    pub punchout: bool,
}

impl Default for VignetteParams {
    fn default() -> Self {
        Self {
            feather: 0.5,
            roundness: 1.0,
            scale: 1.0,
            strength: 0.5,
            tint: 0.0,
            overlaycolor: [0.0, 0.0, 0.0, 1.0],
            punchout: false,
        }
    }
}

/// Parameters for unsharp mask sharpening.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

/// Parameters for lift adjustment / copy background.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LiftParams {
    /// Blend amount (0.0 = 100% background, 1.0 = 100% shape color).
    pub fill: f32,
}

impl Default for LiftParams {
    fn default() -> Self {
        Self { fill: 0.0 }
    }
}

/// Parameters for offset effect.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OffsetParams {
    /// Offset vector in pixels [dx, dy].
    pub offset: [f32; 2],
    /// Feathering amount.
    pub feather: f32,
    /// Whether to use as mask.
    pub mask: bool,
}

impl Default for OffsetParams {
    fn default() -> Self {
        Self {
            offset: [0.0, 0.0],
            feather: 0.0,
            mask: false,
        }
    }
}

/// Parameters for find edges effect.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FindEdgesParams {
    /// Smoothing amount.
    pub smoothing: f32,
    /// Threshold amount.
    pub threshold: f32,
    /// Whether to invert the edge detection output.
    pub invert: bool,
}

impl Default for FindEdgesParams {
    fn default() -> Self {
        Self {
            smoothing: 1.0,
            threshold: 1.0,
            invert: true,
        }
    }
}

/// Parameters for stretch segment effect.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StretchSegmentParams {
    /// Angle of stretch in degrees.
    pub angle: f32,
    /// Stretch amount.
    pub stretch: f32,
    /// Shift offset.
    pub offset: f32,
    /// Smoothing of boundaries.
    pub smooth: f32,
}

impl Default for StretchSegmentParams {
    fn default() -> Self {
        Self {
            angle: 0.0,
            stretch: 0.0,
            offset: 0.0,
            smooth: 0.0,
        }
    }
}

/// Parameters for spin effect.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpinParams {
    /// Revolutions per minute (animated).
    pub rpm: Animated<f32>,
}

impl Default for SpinParams {
    fn default() -> Self {
        Self {
            rpm: Animated::Static(0.0),
        }
    }
}

/// Parameters for swirl effect.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SwirlParams {
    /// Strength of the swirl warp.
    pub strength: f32,
    /// Radius of the swirl warp.
    pub radius: f32,
    /// Exponent of the falloff curve.
    pub exponent: i32,
}

impl Default for SwirlParams {
    fn default() -> Self {
        Self {
            strength: 0.0,
            radius: 0.0,
            exponent: 4,
        }
    }
}

/// Parameters for Colorize effect.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColorizeParams {
    /// Tint vector: [hue_degrees, strength, 0.0]
    pub tint: [f32; 3],
}

impl Default for ColorizeParams {
    fn default() -> Self {
        Self {
            tint: [0.0, 0.0, 0.0],
        }
    }
}

/// Parameters for Wipe transition/mask effect.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WipeParams {
    /// Starting progress of the wipe mask (0.0 to 1.0)
    pub start: Animated<f32>,
    /// Ending progress of the wipe mask (0.0 to 1.0)
    pub end: Animated<f32>,
    /// Angle of the wipe transition in degrees
    pub angle: Animated<f32>,
    /// Softness of the wipe boundary in pixels
    pub feather: f32,
}

impl Default for WipeParams {
    fn default() -> Self {
        Self {
            start: Animated::Static(0.0),
            end: Animated::Static(1.0),
            angle: Animated::Static(0.0),
            feather: 0.0,
        }
    }
}
