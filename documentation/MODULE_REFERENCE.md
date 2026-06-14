# Workspace Module Reference

This document maps out the Cargo workspace structure and individual modules across all packages.

---

## 1. Root Package (`am-renderer`)

- **`Cargo.toml`**: Configures the Cargo workspace containing all packages.
- **`src/main.rs`**: Main CLI executable entry point. Parsed commands map to `info` or `render`.
- **`src/config.rs`**: Handles loader configuration for disabling rendering effects.
- **`src/render/debug_effects.rs`**: Generates side-by-side diagnostic images for isolated effect stacks.

---

## 2. Package `packages/graph-resolver`
Defines the core evaluation domain structures and Bezier curve interpolation.

- **`src/model/project.rs`**: Represents the root project, viewport boundaries, audio, and asset maps.
- **`src/model/layer.rs`**: Represents individual layers, timing bounds, blend modes, and fill options.
- **`src/model/animation.rs`**: Animated keyframe ease curves with Newton-Raphson solvers for cubic bezier control points.
- **`src/model/effect.rs`**: Parameter models and parameter-type structures for all visual effects.
- **`src/eval/timeline.rs`**: Evaluates animation tracks for layers at a given time offset, producing a static `ResolvedScene`.
- **`src/eval/transform.rs`**: Generates 2D affine matrices for spatial translations, scaling, and rotation.

---

## 3. Package `packages/alight-parser`
Translates serialized Alight Motion XML structure to domain models.

- **`src/parser.rs`**: Handles low-level reading of the Alight XML hierarchy.
- **`src/types.rs`**: Direct XML mapping deserializers using `serde`.
- **`src/converter.rs`**: Bridges the gap from XML schemas to internal models. Implements the coordinate system scale converter (`coord_scale = 2.0`) and asset-name auto-pairing.

---

## 4. Package `packages/renderer-core`
Handles pixel processing, composite math, and layer rendering.

- **`src/compositor/mod.rs`**: Orchestrates pixel buffers, handles transparent layout clipping, and sequences layer evaluation.
- **`src/compositor/cache.rs`**: Thread-safe source image and video frame decoding cache.
- **`src/compositor/shape.rs`**: Rasterizes solid fills, gradient maps, and circular shape boundaries.
- **`src/compositor/gradient.rs`**: Renders multi-stop linear color gradients.
- **`src/compositor/utils.rs`**: Conversions for hex color hashes to RGBA float vectors.
- **`src/blending.rs`**: Implements 8 Porter-Duff pixel blend mode formulas (Multiply, Screen, Overlay, etc.).
- **`src/debug_layout.rs`**: Draws bounding contours, pivot coordinates, and diagnostic labeling.
- **`src/effects/mod.rs`**: Main sequencer/dispatcher calling pixel effects.
- **`src/effects/transform.rs`**: Modulates layer matrix offsets before sampling (Oscillate, Swing, RandomDisplace, Spin).
- **`src/effects/`**:
  - `exposure.rs`, `brightness_contrast.rs`, `hsl.rs`, `color_tint.rs`, `colorize.rs`, `vignette.rs` (Color Adjustments)
  - `tile.rs`, `offset.rs`, `stretch_segment.rs`, `swirl.rs`, `wipe.rs` (Coordinates Modifiers)
  - `gaussian_blur.rs`, `lens_blur.rs`, `sharpen.rs` (Convolutions)
  - `find_edges.rs` (Sobel Edges)
  - `lift.rs` (Shadows & Copy Background)
  - `luma_key.rs` (Transparency keying)

---

## 5. Package `packages/export-service`
Manages parallel batch exporting.

- **`src/png.rs`**: Exports sequences of images concurrently using a multi-threaded `rayon` bridge.
- **`src/video.rs`**: Pipelines audio tracks, loads caches, and invokes `ffmpeg` to compile sequences into an H.264 MP4.

---

## 6. Package `packages/preview-service`
Provides the collaborative API and real-time preview daemon.

- **`src/main.rs`**: Starts the Axum web daemon, routes API commands, registers WebSocket paths, and loads SQLite databases.
- **`src/db.rs`**: SQLite ORM queries and database table migration definitions.
- **`src/mutations.rs`**: Handles transactional mutations (create, update, delete layers, undo, redo actions).
- **`src/scheduler.rs`**: Animates, renders, WebP-compresses, and streams frames down WebSocket clients.

---

## 7. Package `packages/web-editor`
React frontend editing canvas application.

- **`src/App.tsx`**: Main component staging layout, tracks timeline position, and shows action triggers.
- **`src/components/CanvasViewer.tsx`**: Visualizes WebSocket WebP streams in real-time.
- **`src/components/Timeline.tsx`**: Renders layer timing boundaries, zoom control, and selection tools.
- **`src/components/PropertyPanel.tsx`**: Visual controls for updating scale, coordinate values, and active effects.

---

## 8. Package `packages/integration-tests`
Houses E2E workload integration validation tests.

- **`tests/`**: Contains E2E tests validating REST APIs, WebSocket frames, undo/redo states, database transactions, and memory leaks.
