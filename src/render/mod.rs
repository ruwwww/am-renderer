//! Render module — software rendering pipeline.
//!
//! Contains:
//! - [`compositor`] — main scene compositor
//! - [`blending`] — blend mode implementations
//! - [`effects`] — render-time effects (color, blur, UV)
//! - [`debug_effects`] — effect isolation debug rendering

pub mod blending;
pub mod compositor;
pub mod debug_effects;
pub mod debug_layout;
pub mod effects;

pub use blending::blend_pixel;
pub use compositor::{parse_hex_color, render_scene, ImageCache};
