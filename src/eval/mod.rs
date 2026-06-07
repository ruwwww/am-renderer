//! Evaluation module — resolves animated properties at specific times.
//!
//! This module contains:
//! - [`timeline`] — evaluates a project at a specific time to produce resolved scenes
//! - [`transform`] — builds affine transform matrices from layer properties
//! - [`effects`] — applies transform-modifying effects

pub mod timeline;
pub mod transform;
pub mod effects;

pub use timeline::{evaluate, ResolvedScene};
