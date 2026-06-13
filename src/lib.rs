//! Public API adapter for am-renderer crate.
//!
//! Re-exports modules from sub-packages to preserve backward compatibility.

pub mod config;

pub use alight_parser as parser;
pub use graph_resolver::eval;
pub use graph_resolver::model;
pub use renderer_core as render;
pub use export_service as export;
