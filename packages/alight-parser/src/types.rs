//! Raw XML deserialization types for Alight Motion project files.
//!
//! These types map directly to the XML structure and should not be used
//! outside the parser module. They are converted to the internal project
//! model before being consumed by the evaluator or renderer.

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Keyframe
// ---------------------------------------------------------------------------

/// A single keyframe element: `<kf t="0.5" v="1.0,2.0" e="cubicBezier ..."/>`.
///
/// - `t` is the *normalised* time (0.0–1.0 within the layer's duration).
/// - `v` is the value, encoded as a comma-separated string.
/// - `e` is an optional easing descriptor such as
///   `"cubicBezier 0.42 0.0 1.0 1.0"` or `"local cubicBezier 0.0 0.0 0.58 1.0"`.
#[derive(Debug, Clone, Deserialize)]
pub struct XmlKeyframe {
    /// Normalised time within the layer (0.0 = start, 1.0 = end).
    #[serde(rename = "@t")]
    pub t: String,

    /// Value at this keyframe, as a raw string (e.g. `"1.0"`, `"540.0,960.0"`).
    #[serde(rename = "@v")]
    pub v: String,

    /// Optional easing descriptor.
    #[serde(rename = "@e", default)]
    pub e: Option<String>,
}

// ---------------------------------------------------------------------------
// Animated property helpers (Vec3 / Vec2 / Float)
// ---------------------------------------------------------------------------

/// An animated or static **Vec3** property (used for `<location>`).
///
/// Can appear in the XML as either:
/// - Static: `<location value="540.0,960.0,0.0"/>`
/// - Animated: `<location><kf .../><kf .../></location>`
#[derive(Debug, Clone, Deserialize)]
pub struct XmlAnimatedVec3 {
    /// Static value (present when there are no keyframes).
    #[serde(rename = "@value", default)]
    pub value: Option<String>,

    /// Keyframes (present when the property is animated).
    #[serde(rename = "kf", default)]
    pub keyframes: Vec<XmlKeyframe>,
}

/// An animated or static **Vec2** property (used for `<scale>`).
///
/// Same dual representation as [`XmlAnimatedVec3`].
#[derive(Debug, Clone, Deserialize)]
pub struct XmlAnimatedVec2 {
    /// Static value (present when there are no keyframes).
    #[serde(rename = "@value", default)]
    pub value: Option<String>,

    /// Keyframes (present when the property is animated).
    #[serde(rename = "kf", default)]
    pub keyframes: Vec<XmlKeyframe>,
}

/// An animated or static **f32** property (used for `<rotation>`, `<opacity>`).
///
/// Same dual representation as [`XmlAnimatedVec3`].
#[derive(Debug, Clone, Deserialize)]
pub struct XmlAnimatedFloat {
    /// Static value (present when there are no keyframes).
    #[serde(rename = "@value", default)]
    pub value: Option<String>,

    /// Keyframes (present when the property is animated).
    #[serde(rename = "kf", default)]
    pub keyframes: Vec<XmlKeyframe>,
}

// ---------------------------------------------------------------------------
// Transform
// ---------------------------------------------------------------------------

/// The `<transform>` element containing location, scale, rotation, opacity.
#[derive(Debug, Clone, Deserialize)]
pub struct XmlTransform {
    /// Translation in scene coordinates (x, y, z).
    #[serde(default)]
    pub location: Option<XmlAnimatedVec3>,

    /// Scale factor (x, y).
    #[serde(default)]
    pub scale: Option<XmlAnimatedVec2>,

    /// Rotation in degrees.
    #[serde(default)]
    pub rotation: Option<XmlAnimatedFloat>,

    /// Opacity (0.0–1.0).
    #[serde(default)]
    pub opacity: Option<XmlAnimatedFloat>,
}

// ---------------------------------------------------------------------------
// Effect & Property
// ---------------------------------------------------------------------------

/// An effect property: `<property name="mirror" type="bool" value="true"/>` or
/// with animated keyframes as children.
#[derive(Debug, Clone, Deserialize)]
pub struct XmlProperty {
    /// Property name (e.g. `"mirror"`, `"freq"`, `"mag"`, `"size"`).
    #[serde(rename = "@name")]
    pub name: String,

    /// Type hint from the XML (e.g. `"bool"`, `"float"`, `"vec2"`).
    #[serde(rename = "@type", default)]
    pub r#type: Option<String>,

    /// Static value (present when there are no child keyframes).
    #[serde(rename = "@value", default)]
    pub value: Option<String>,

    /// Keyframes (present when the property is animated).
    #[serde(rename = "kf", default)]
    pub keyframes: Vec<XmlKeyframe>,
}

/// An `<effect>` element that modifies a shape.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlEffect {
    /// Fully-qualified effect identifier (e.g. `"com.alightcreative.effects.tile"`).
    #[serde(rename = "@id")]
    pub id: String,

    /// Whether this effect is applied locally.
    #[serde(rename = "@locallyApplied", default)]
    pub locally_applied: Option<String>,

    /// Effect parameters.
    #[serde(rename = "property", default)]
    pub properties: Vec<XmlProperty>,
}

// ---------------------------------------------------------------------------
// Fill Color & Gradient
// ---------------------------------------------------------------------------

/// A `<fillColor value="#FF000000"/>` element.
#[derive(Debug, Clone, Deserialize)]
pub struct XmlFillColor {
    /// ARGB hex colour string.
    #[serde(rename = "@value")]
    pub value: String,
}

/// A `<gradient>` element describing a colour gradient fill.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlGradient {
    /// Gradient type (e.g. `"linear"`, `"radial"`).
    #[serde(rename = "@type", default)]
    pub r#type: Option<String>,

    /// Start colour in ARGB hex.
    #[serde(rename = "@startColor", default)]
    pub start_color: Option<String>,

    /// End colour in ARGB hex.
    #[serde(rename = "@endColor", default)]
    pub end_color: Option<String>,

    /// Start position as `"x,y"` in normalised [0,1] coordinates.
    #[serde(rename = "@start", default)]
    pub start: Option<String>,

    /// End position as `"x,y"` in normalised [0,1] coordinates.
    #[serde(rename = "@end", default)]
    pub end: Option<String>,
}

// ---------------------------------------------------------------------------
// Gain (audio)
// ---------------------------------------------------------------------------

/// The `<gain>` element inside `<audio>`, containing keyframes.
#[derive(Debug, Clone, Deserialize)]
pub struct XmlGain {
    /// Keyframes controlling gain over time.
    #[serde(rename = "kf", default)]
    pub keyframes: Vec<XmlKeyframe>,
}

// ---------------------------------------------------------------------------
// Audio
// ---------------------------------------------------------------------------

/// An `<audio>` element representing an audio layer.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlAudio {
    /// Unique layer id.
    #[serde(rename = "@id")]
    pub id: String,

    /// User-visible label.
    #[serde(rename = "@label", default)]
    pub label: Option<String>,

    /// Start time in the scene timeline (milliseconds).
    #[serde(rename = "@startTime")]
    pub start_time: String,

    /// End time in the scene timeline (milliseconds).
    #[serde(rename = "@endTime")]
    pub end_time: String,

    /// URI pointing to the audio asset.
    #[serde(rename = "@src", default)]
    pub src: Option<String>,

    /// In-point within the source media (milliseconds).
    #[serde(rename = "@inTime", default)]
    pub in_time: Option<String>,

    /// Out-point within the source media (milliseconds).
    #[serde(rename = "@outTime", default)]
    pub out_time: Option<String>,

    /// Gain envelope.
    #[serde(default)]
    pub gain: Option<XmlGain>,
}

// ---------------------------------------------------------------------------
// Shape
// ---------------------------------------------------------------------------

/// A `<shape>` element representing a visual layer.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlShape {
    /// Unique layer id.
    #[serde(rename = "@id")]
    pub id: String,

    /// User-visible label.
    #[serde(rename = "@label", default)]
    pub label: Option<String>,

    /// Start time in the scene timeline (milliseconds).
    #[serde(rename = "@startTime")]
    pub start_time: String,

    /// End time in the scene timeline (milliseconds).
    #[serde(rename = "@endTime")]
    pub end_time: String,

    /// Fill type (e.g. `"media"`, `"color"`, `"gradient"`).
    #[serde(rename = "@fillType", default)]
    pub fill_type: Option<String>,

    /// URI of the fill image when `fillType == "media"`.
    #[serde(rename = "@fillImage", default)]
    pub fill_image: Option<String>,

    /// How the media fills the shape (e.g. `"stretch"`, `"fit"`, `"fill"`).
    #[serde(rename = "@mediaFillMode", default)]
    pub media_fill_mode: Option<String>,

    /// Shape primitive (e.g. `".rect"`, `".ellipse"`, `".triangle"`).
    #[serde(rename = "@s", default)]
    pub s: Option<String>,

    /// Blending mode (e.g. `"multiply"`, `"overlay"`, `"screen"`).
    #[serde(rename = "@blending", default)]
    pub blending: Option<String>,

    /// Whether this shape is hidden.
    #[serde(rename = "@hidden", default)]
    pub hidden: Option<String>,

    // -- child elements ------------------------------------------------------
    /// Transform (location / scale / rotation / opacity).
    #[serde(default)]
    pub transform: Option<XmlTransform>,

    /// Applied effects.
    #[serde(rename = "effect", default)]
    pub effects: Vec<XmlEffect>,

    /// Fill colour.
    #[serde(rename = "fillColor", default)]
    pub fill_color: Option<XmlFillColor>,

    /// Gradient fill.
    #[serde(default)]
    pub gradient: Option<XmlGradient>,

    /// Top-level shape properties (e.g. `<property name="size" ...>`).
    #[serde(rename = "property", default)]
    pub properties: Vec<XmlProperty>,
}

// ---------------------------------------------------------------------------
// Media & Bookmark
// ---------------------------------------------------------------------------

/// A `<media>` element describing an imported asset.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlMedia {
    /// URI of the asset (e.g. `"am-internal:///HASH.PNG"`).
    #[serde(rename = "@uri")]
    pub uri: String,

    /// Original filename.
    #[serde(rename = "@filename", default)]
    pub filename: Option<String>,

    /// Title / display name.
    #[serde(rename = "@title", default)]
    pub title: Option<String>,

    /// MIME type (e.g. `"image/png"`, `"audio/mpeg"`).
    #[serde(rename = "@type", default)]
    pub r#type: Option<String>,

    /// Duration of the media in microseconds (for audio/video).
    #[serde(rename = "@duration", default)]
    pub duration: Option<String>,

    /// File size in bytes.
    #[serde(rename = "@size", default)]
    pub size: Option<String>,

    /// Content signature / hash.
    #[serde(rename = "@sig", default)]
    pub sig: Option<String>,

    /// Timestamp when media info was last updated.
    #[serde(rename = "@infoUpdated", default)]
    pub info_updated: Option<String>,

    /// Width in pixels (for images / video).
    #[serde(rename = "@width", default)]
    pub width: Option<String>,

    /// Height in pixels (for images / video).
    #[serde(rename = "@height", default)]
    pub height: Option<String>,
}

/// A `<bookmark t="5416"/>` element.
#[derive(Debug, Clone, Deserialize)]
pub struct XmlBookmark {
    /// Bookmark time in milliseconds.
    #[serde(rename = "@t")]
    pub t: String,
}

// ---------------------------------------------------------------------------
// Scene (root)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub enum XmlSceneElement {
    #[serde(rename = "media")]
    Media(XmlMedia),
    #[serde(rename = "bookmark")]
    Bookmark(XmlBookmark),
    #[serde(rename = "audio")]
    Audio(XmlAudio),
    #[serde(rename = "shape")]
    Shape(XmlShape),
    #[serde(other)]
    Unknown,
}

/// Root `<scene>` element – the top-level container for an Alight Motion project.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlScene {
    /// Project title.
    #[serde(rename = "@title", default)]
    pub title: Option<String>,

    /// Canvas width in pixels.
    #[serde(rename = "@width")]
    pub width: String,

    /// Canvas height in pixels.
    #[serde(rename = "@height")]
    pub height: String,

    /// Export width in pixels.
    #[serde(rename = "@exportWidth", default)]
    pub export_width: Option<String>,

    /// Export height in pixels.
    #[serde(rename = "@exportHeight", default)]
    pub export_height: Option<String>,

    /// Background colour in ARGB hex.
    #[serde(rename = "@bgcolor", default)]
    pub bgcolor: Option<String>,

    /// Total scene duration in milliseconds.
    #[serde(rename = "@totalTime")]
    pub total_time: String,

    /// Frames per second.
    #[serde(rename = "@fps")]
    pub fps: String,

    /// Timestamp of last modification.
    #[serde(rename = "@modifiedTime", default)]
    pub modified_time: Option<String>,

    /// Alight Motion internal version code.
    #[serde(rename = "@amver", default)]
    pub amver: Option<String>,

    /// File format version code.
    #[serde(rename = "@ffver", default)]
    pub ffver: Option<String>,

    /// Application identifier with version (e.g. `"com.alightcreative.motion/6.2.6"`).
    #[serde(rename = "@am", default)]
    pub am: Option<String>,

    /// Platform the project was created on (`"ios"` / `"android"`).
    #[serde(rename = "@amplatform", default)]
    pub amplatform: Option<String>,

    /// Pre-compose mode (e.g. `"dynamicResolution"`).
    #[serde(rename = "@precompose", default)]
    pub precompose: Option<String>,

    /// Retime mode (e.g. `"freeze"`).
    #[serde(rename = "@retime", default)]
    pub retime: Option<String>,

    // -- child elements ------------------------------------------------------
    /// Scene elements.
    #[serde(rename = "$value", default)]
    pub elements: Vec<XmlSceneElement>,
}

impl XmlScene {
    /// Get references to all `<media>` elements.
    pub fn media(&self) -> Vec<&XmlMedia> {
        self.elements
            .iter()
            .filter_map(|e| match e {
                XmlSceneElement::Media(m) => Some(m),
                _ => None,
            })
            .collect()
    }

    /// Get references to all `<bookmark>` elements.
    pub fn bookmarks(&self) -> Vec<&XmlBookmark> {
        self.elements
            .iter()
            .filter_map(|e| match e {
                XmlSceneElement::Bookmark(b) => Some(b),
                _ => None,
            })
            .collect()
    }

    /// Get references to all `<audio>` elements.
    pub fn audio(&self) -> Vec<&XmlAudio> {
        self.elements
            .iter()
            .filter_map(|e| match e {
                XmlSceneElement::Audio(a) => Some(a),
                _ => None,
            })
            .collect()
    }

    /// Get references to all `<shape>` elements.
    pub fn shapes(&self) -> Vec<&XmlShape> {
        self.elements
            .iter()
            .filter_map(|e| match e {
                XmlSceneElement::Shape(s) => Some(s),
                _ => None,
            })
            .collect()
    }
}
