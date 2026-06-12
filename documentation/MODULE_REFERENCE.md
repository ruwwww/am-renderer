# Module Reference

## `src/lib.rs`
Library root. Re-exports the five top-level modules: `parser`, `model`, `eval`, `render`, `export`.

## `src/main.rs` (909 lines)
CLI entry point and XML-to-model converter.

**CLI Commands:**
- `am-renderer info -i <file>` - Print project metadata
- `am-renderer render -i <file> -a <assets> -o <output>` - Render frame(s)/video

**Key functions:**
- `convert_project(xml) -> Project` - Converts raw XML types to domain model
- `convert_effect(xml_effect) -> Effect` - Maps 25+ XML effect IDs to `EffectType` variants
- `build_virtual_mappings()` - Auto-pairs media URIs to asset files via round-robin

## `src/parser/` — XML Deserialization

### `mod.rs`
Module declarations.

### `types.rs` (447 lines)
Raw XML deserialization types via `#[derive(serde::Deserialize)]`:
- `XmlScene` — Root element with dimensions, duration, FPS, layers, media, audio
- `XmlShape`, `XmlTransform` — Layer shape and transform
- `XmlEffect` — Effect with name map and properties list
- `XmlProperty` — Property with optional animated value or keyframes
- `XmlKeyframe` — Individual keyframe with value, easing, time
- `XmlMedia`, `XmlAudio` — Media references
- `XmlGradient`, `XmlGradientStop` — Gradient definitions

All XML attribute mappings use `#[serde(rename = "@attr_name")]`.

### `xml.rs` (272 lines)
- `parse_xml(path: &Path) -> Result<XmlScene>` — Reads and deserializes an XML file
- 4 unit tests covering: minimal scene, full scene, static transform children, multi-layer with audio/media

## `src/model/` — Domain Model

### `mod.rs`
Module declarations.

### `project.rs` (74 lines)
- `Project` — Title, dimensions, background color, duration, FPS, media refs, audio tracks, layers
- `Project::duration_secs()` — Duration in seconds
- `Project::total_frames()` — Total frame count
- `MediaRef` — Media item with ID, filename, virtual URI
- `AudioTrack` — Audio clip with source, timing, volume

### `layer.rs` (153 lines)
- `Layer` — Source layer with ID, name, timing, visibility, transform, fill type, effects
- `ResolvedLayer` — Evaluated layer with concrete (non-animated) values
- `LayerTransform` — Anchor, position, scale, rotation, opacity (all `Animated<f32>`)
- `BlendMode` — Enum: Normal, Multiply, Screen, Overlay, Darken, Lighten, Subtract, Add
- `FillType` — Enum: None, Media(String), Color([f32;4]), Gradient(Gradient)
- `Gradient`, `GradientStop` — Linear gradient with stops

### `animation.rs` (152 lines)
Core animation system:
- `Animated<T>` — Enum: `Static(T)` or `Keyframed(Vec<Keyframe<T>>)`
- `Keyframe<T>` — Value, easing type, time (normalized 0-1)
- `EasingType` — `Linear` or `CubicBezier(f32, f32, f32, f32)`
- `Lerp` trait — Implemented for `f32`, `[f32;2]`, `[f32;3]`, `[f32;4]`
- `Animated::evaluate(normalized_time)` — Interpolates between keyframes with cubic bezier easing (Newton-Raphson solver)

### `effect.rs` (538 lines)
- `Effect` — Type tag and parameter struct
- `EffectType` — Enum with 25+ variants (see EFFECTS_CATALOG.md)
- Each effect has a dedicated parameter struct (e.g., `OscillateParams`, `MotionBlurParams`, `GaussianBlurParams`)

## `src/eval/` — Timeline Evaluation

### `mod.rs`
Module declarations.

### `timeline.rs` (115 lines)
- `evaluate(project, time_secs) -> ResolvedScene` — Main evaluation function
  - Filters visible layers within time range
  - Computes normalized time per layer
  - Evaluates all animated properties
  - Applies fade effects to opacity (delegates to `render/effects/fade.rs`)

### `transform.rs` (102 lines)
- `build_transform_matrix(transform) -> Mat3` — Builds 3x3 affine matrix (translate * rotate * scale) with anchor-point correction
- `invert_transform(transform) -> Mat3` — Inverts the affine matrix
- `transform_point(mat, point) -> Vec2` — Matrix * vector multiplication

Transform-modifying effects (Oscillate, Swing, RandomDisplace) were moved to `render/effects/transform.rs`.

## `src/render/` — Software Rendering Pipeline

### `mod.rs`
Module declarations.

### `compositor.rs` (907 lines)
Core rendering engine:
- `render_scene(project, resolved_scene, cache, debug) -> RgbaImage` — Creates canvas, fills background, composites layers bottom-to-top
- `render_layer(layer, cache, bg, debug) -> RgbaImage` — Renders single layer:
  - Handles FillType::Media (loads/stretches image), FillType::Color (solid fill), FillType::Gradient (linear gradient)
  - Uses inverse-transform sampling per pixel (parallelized with rayon)
  - Chains pixel-space effects
  - Supports Lift (Copy Background) with affine-stepped sampling
- `ImageCache` — Virtual URI to file path mapping with `std::sync::Arc<Mutex<HashMap>>`
- `parse_hex_color(hex) -> [f32;4]` — Converts hex color strings to RGBA
- Debug layout mode: bounding boxes, pixel-font layer labels, edge detection overlays
- Per-row parallelism: `.par_bridge()` on row iterator

### `blending.rs` (95 lines)
- `blend_pixel(base, overlay, mode) -> Rgba` — Implements 8 blend modes using Porter-Duff "over" compositing

### `effects/`
#### `mod.rs` — Centralized pixel effect dispatcher
- `apply_pixel_effects(effects, img, layer) -> RgbaImage` — Dispatches all pixel-processing effects in order via a single match, evaluating animated parameters using the layer's normalized time
- Also re-exports `apply_transform_effects` from `transform.rs`

#### Per-effect modules (one file each):
- `exposure.rs` — Exposure adjustment in EV stops
- `brightness_contrast.rs` — Brightness/contrast with contrast pivot
- `hsl.rs` — Full HSL adjustment (hue shift, saturation, lightness) with `rgb_to_hsl`/`hsl_to_rgb` helpers
- `color_tint.rs` — Color fill with alpha blending
- `vignette.rs` — Radial darkening at frame edges with punchout mode
- `find_edges.rs` — Sobel edge detection via parallel channel decomposition
- `highlight_shadow.rs` — Stub (not yet implemented)
- `gradient_overlay.rs` — Stub (not yet implemented)
- `luma_key.rs` — Stub (not yet implemented)
- `tile.rs` — Repeating/mirror tiling with brick stagger and per-tile rotation
- `offset.rs` — Scroll/shift UV with wrapping, plus `sample_wrapped`/`sample_clamped` helpers
- `stretch_segment.rs` — Split frame and stretch a contiguous segment
- `gaussian_blur.rs` — Separable 2-pass Gaussian blur with kernel builder; also exports `box_blur`
- `lens_blur.rs` — Stub (not yet implemented)
- `sharpen.rs` — Stub (not yet implemented)
- `lift.rs` — Copy Background: samples the composition canvas via forward transform with affine stepping, blends with optional shape fill
- `transform.rs` — Transform-modifying effects: Oscillate (sinusoidal displacement), Swing (pendulum rotation), RandomDisplace (deterministic noise displacement); moved from `eval/effects.rs`
- `fade.rs` — Linear opacity fade at layer boundaries; called from `eval/timeline.rs`
- `blink.rs` — Stub (periodic visibility)
- `motion_blur.rs` — Stub (temporal supersampling)
- `gaussian_blur.rs` — Separable 2-pass Gaussian and box blur (originally `blur.rs`)
- `color.rs` — **Deleted**: split into individual modules above
- `blur.rs` — **Renamed**: to `gaussian_blur.rs`
- `uv.rs` — **Deleted**: split into individual modules above

## `src/export/` — Output Formats

### `mod.rs`
Module declarations.

### `png.rs` (81 lines)
- `export_frame(path, img)` — Save single RGBA image as PNG
- `export_sequence(project, cache, output_dir)` — Render all frames in parallel using rayon:
  1. Pre-load all media assets into cache
  2. Par-iterate frames from 0 to total_frames
  3. Call `render_scene` then `export_frame` per frame

### `video.rs` (49 lines)
- `export_mp4(frame_dir, output_path, fps)` — Shells out to ffmpeg:
  - Reads `frame_%06d.png` from frame directory
  - Encodes H.264 with yuv420p pixel format
  - Requires ffmpeg to be installed and on PATH

## Examples

### `examples/analyze_presets.rs`
Reads all XML files from a presets directory and prints shape properties found in each.

### `examples/auto_pair.rs`
Scans a source directory for image files and copies/renames them to match the virtual URIs expected by an XML project file.

### `examples/test_sizing.rs`
Loads images from the assets directory and prints their dimensions.
