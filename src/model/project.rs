//! Project-level model types.

use super::layer::Layer;

/// Top-level project model parsed from an Alight Motion XML file.
#[derive(Debug, Clone)]
pub struct Project {
    /// Project title.
    pub title: Option<String>,
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Export width in pixels.
    pub export_width: u32,
    /// Export height in pixels.
    pub export_height: u32,
    /// Background color as RGBA [0.0–1.0].
    pub bg_color: [f32; 4],
    /// Total duration in milliseconds.
    pub total_time: f32,
    /// Frames per second.
    pub fps: f32,
    /// Imported media assets.
    pub media: Vec<MediaRef>,
    /// Audio tracks.
    pub audio_tracks: Vec<AudioTrack>,
    /// Visual layers (shapes), ordered bottom-to-top.
    pub layers: Vec<Layer>,
}

/// A reference to an imported media asset.
#[derive(Debug, Clone)]
pub struct MediaRef {
    /// URI of the asset (e.g. `"am-internal:///HASH.PNG"`).
    pub uri: String,
    /// Original filename.
    pub filename: Option<String>,
    /// Display title.
    pub title: Option<String>,
    /// MIME type.
    pub mime_type: Option<String>,
    /// Width in pixels (for images / video).
    pub width: Option<u32>,
    /// Height in pixels (for images / video).
    pub height: Option<u32>,
}

/// An audio track in the project.
#[derive(Debug, Clone)]
pub struct AudioTrack {
    /// Unique id.
    pub id: u64,
    /// User-visible label.
    pub label: Option<String>,
    /// Start time in milliseconds.
    pub start_time: f32,
    /// End time in milliseconds.
    pub end_time: f32,
    /// Source media URI.
    pub src: Option<String>,
}

impl Project {
    /// Total duration in seconds.
    pub fn duration_secs(&self) -> f32 {
        self.total_time / 1000.0
    }

    /// Total number of frames based on duration and FPS.
    pub fn total_frames(&self) -> u32 {
        (self.duration_secs() * self.fps).ceil() as u32
    }
}
