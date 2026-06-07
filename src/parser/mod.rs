//! Parser module for reading Alight Motion XML project files.
//!
//! Re-exports the raw XML deserialization types and the [`parse_xml`] function.

pub mod types;
pub mod xml;

pub use types::*;
pub use xml::parse_xml;
