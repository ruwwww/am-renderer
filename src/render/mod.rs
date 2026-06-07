//! Render module — software rendering pipeline.
//!
//! Contains:
//! - [`compositor`] — main scene compositor
//! - [`blending`] — blend mode implementations
//! - [`effects`] — render-time effects (color, blur, UV)

pub mod compositor;
pub mod blending;
pub mod effects;

pub use compositor::{render_scene, ImageCache, parse_hex_color};
pub use blending::blend_pixel;
