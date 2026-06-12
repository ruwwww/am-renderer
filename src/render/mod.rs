//! Render module — software rendering pipeline.
//!
//! Contains:
//! - [`compositor`] — main scene compositor
//! - [`blending`] — blend mode implementations
//! - [`effects`] — render-time effects (color, blur, UV)
//! - [`debug_effects`] — effect isolation debug rendering

pub mod compositor;
pub mod blending;
pub mod effects;
pub mod debug_effects;
pub mod debug_layout;

pub use compositor::{render_scene, ImageCache, parse_hex_color};
pub use blending::blend_pixel;

