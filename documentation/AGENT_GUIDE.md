# AI Agent Guide

This document provides structured context for AI agents working with the `am-renderer` codebase.

## Quick Facts

- **Language**: Rust (edition 2021)
- **Build system**: Cargo
- **Key crates**: `quick-xml` (parsing), `image` (rendering), `glam` (math), `clap` (CLI), `rayon` (parallelism)
- **Entry point**: `src/main.rs` (CLI driver)
- **Library root**: `src/lib.rs` (re-exports 5 modules)
- **Tests**: Inline `#[cfg(test)] mod tests` in `src/parser/xml.rs` and `src/render/effects/transform.rs`
- **External dep**: `ffmpeg` (required for MP4 export only)

## Pipeline (data flow)

```
XML → parser → XmlScene → parser::converter::convert_project → Project
  → eval::timeline::evaluate(t) → ResolvedScene
  → render::compositor::render_scene() → RgbaImage
  → export::png or export::video → file
```

## Where to Add New Effects

1. **Define XML type** → `src/parser/types.rs` if new XML elements are needed
2. **Define parameter struct** → `src/model/effect.rs` — add variant to `EffectType` enum and create params struct
3. **Add XML-to-model conversion** → `src/parser/converter.rs` in `convert_effect()`
4. **If transform modifier**: implement in `src/render/effects/transform.rs`
5. **If pixel/image effect**: create a new file in `src/render/effects/` (e.g., `my_effect.rs`) and add a match arm to `apply_pixel_effects()` in `src/render/effects/mod.rs`
6. **Test**: add unit tests alongside the implementation

## Animation System

- `Animated<T>` is the core type: either `Static(value)` or `Keyframed(vec)`
- `evaluate(normalized_time)` interpolates keyframes with cubic bezier easing
- Normalized time is per-layer: `(layer_time - layer_start) / layer_duration`
- `Lerp` trait must be implemented for any type used as `Animated<T>`
- Already implemented: `f32`, `[f32;2]`, `[f32;3]`, `[f32;4]`

## Rendering Pipeline (per layer, per pixel)

1. Build affine transform matrix from layer transform
2. For each output pixel (parallelized by row):
   a. Compute inverse transform to find source UV
   b. Sample source image (wrapped or clamped)
   c. Apply UV effects (tile, offset, stretch)
   d. Apply color effects (exposure, HSL, vignette, etc.)
   e. Blend onto background using layer blend mode
3. Apply full-image blurs (Gaussian, LensBlur) after layer is rendered
4. Composite onto canvas with Porter-Duff "over"

## Debug Mode

- `--debug-layout` flag enables debug rendering
- Draws bounding box outlines, pivot crosshairs, and layer metadata labels over an adaptive canvas (expanding to fit off-screen layers, with 60% dimming outside project boundaries)
- Drawing routines are isolated in `src/render/debug_layout.rs`
- Controlled by `debug` parameter in `render_scene()` and `render_layer()`

## Auto-Pairing

When `--auto-pair` is enabled, `build_virtual_mappings()` performs round-robin assignment of asset files to XML virtual URIs. This is useful when the XML references media that was renamed/moved after export.

## Common Patterns

- **Error handling**: `anyhow::Result` and `anyhow::Context` throughout
- **Image type**: `image::RgbaImage` (u8, 4 channels)
- **Color representation**: `[f32; 4]` for RGBA in 0.0–1.0 range in the render pipeline
- **Pixel format**: `image::Rgba<u8>` for storage, converted to `[f32; 4]` for processing
- **Parallelism**: `rayon::par_bridge()` on row iterators; `par_iter()` on frame ranges
- **Math types**: `glam::Vec2`, `glam::Mat3` for 2D affine transforms

## Build & Test Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build (LTO, panic=abort)
cargo test                     # Run all tests
cargo test parser              # Run parser tests only
cargo test effects             # Run effects tests only
cargo run -- info -i presets/preset1.xml
cargo run -- render -i presets/preset1.xml -a assets -o output.mp4
cargo run --example analyze_presets
```

## Code Style

- 4-space indentation
- `snake_case` for functions/variables, `CamelCase` for types/enums
- Doc comments (`///` and `//!`) on all public items
- `#[must_use]` on functions returning `Result` or values that should not be discarded
- `use` statements grouped: std → external crates → local modules
- Modules use `mod.rs` pattern (e.g., `src/render/effects/mod.rs`)
- Avoid unwrap/expect in library code; use `anyhow` context instead
