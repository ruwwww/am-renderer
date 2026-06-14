# Architecture

## Overview

`am-renderer` is modularized as a multi-package Rust workspace + React frontend, separating core scene parsing, evaluation, pixel composition, network preview services, and interactive editing UI.

```
                  ┌─────────────────────────────────────────┐
                  │          Alight Motion XML              │
                  └────────────────────┬────────────────────┘
                                       │
                                       ▼ [alight-parser]
                  ┌─────────────────────────────────────────┐
                  │       Domain model (Project, Layer)     │
                  └────────────────────┬────────────────────┘
                                       │
                                       ▼ [graph-resolver]
                  ┌─────────────────────────────────────────┐
                  │     ResolvedScene (Time-evaluated)      │
                  └────────────────────┬────────────────────┘
                                       │
                                       ▼ [renderer-core]
                  ┌─────────────────────────────────────────┐
                  │            RgbaImage Canvas             │
                  └──────────┬───────────────────┬──────────┘
                             │                   │
      [export-service]       ▼                   ▼ [preview-service]
                  ┌─────────────────┐     ┌─────────────────┐
                  │ PNG / MP4 Files │     │ WebP WS Stream  │
                  └─────────────────┘     └────────┬────────┘
                                                   │
                                                   ▼ [web-editor]
                                          ┌─────────────────┐
                                          │ React Front-end │
                                          └─────────────────┘
```

## Workspace Crate Breakdown

1. **`graph-resolver`**: Holds the core scene evaluation logic and animatable data structures.
   - Defines `Project`, `Layer`, `Effect`, and `Animated<T>`.
   - `evaluate(time_secs) -> ResolvedScene` is stateless, deterministic, and free of side effects.
2. **`alight-parser`**: Converts raw deserialized XML scene models to `graph-resolver` domain models.
   - Applies the fixed coordinate scaling factor `coord_scale = 2.0` (logical points to canvas pixels).
3. **`renderer-core`**: High-performance CPU-based renderer using software rasterization.
   - Composites layers bottom-to-top using Porter-Duff blend modes.
   - Computes sub-pixel transforms via inverse-transform mapping.
   - Implements per-pixel and full-image separable visual effects.
4. **`export-service`**: Implements offline rendering pipelines.
   - Exports frames as individual PNG files in parallel using `rayon`.
   - Encodes PNG sequences to H.264 MP4 videos using `ffmpeg` sub-processes.
5. **`preview-service`**: A stateful Axum API server and WebSocket preview provider.
   - Keeps track of projects, layers, and properties inside an SQLite database (`db.sqlite`).
   - Implements transactional API routes for live mutation (e.g. updating position, opacity, effects).
   - Manages memory-based undo/redo transaction stacks.
   - Employs a stateful frame streamer (`PlaybackScheduler`) that evaluates, renders, WebP-compresses, and broadcasts frames to active WebSocket connections in real-time.
6. **`web-editor`**: An interactive React + Vite frontend timeline editor.
   - Connects to the `preview-service` WebSocket endpoint for frame stream rendering.
   - Calls REST endpoints to perform live layer/property updates, undo, and redo.

## Frame Stream Rendering & Playback Scheduler

The `PlaybackScheduler` in `packages/preview-service/src/scheduler.rs` governs real-time playback:
- Uses a background worker thread with a tick rate matching the project FPS.
- On each tick, if playing, it updates the virtual timeline time, evaluates the project scene via `graph-resolver`, renders the frame via `renderer-core`, compresses it to WebP format, and sends the raw binary frame over a broadcast channel.
- Handles pause, play, seek, and FPS configuration commands.
- Clamps FPS to `[0.1, 240.0]` to avoid zero or negative delta time stepping panics.
- Aborts active rendering tasks when playback pauses or WebSocket clients disconnect.

## Effect Pipeline Flow

Effects are classified by their execution phase in `packages/renderer-core/src/render/effects`:

1. **Transform Modifiers** (`transform.rs`): Applied to the affine transformation matrix *before* inverse sampling (e.g., Oscillate, Swing, RandomDisplace).
2. **UV Effects** (`tile.rs`, `offset.rs`, `stretch_segment.rs`, `swirl.rs`, `wipe.rs`): Alter coordinates during pixel sampling.
3. **Color/Pixel Effects** (`exposure.rs`, `brightness_contrast.rs`, `hsl.rs`, `color_tint.rs`, `vignette.rs`, `find_edges.rs`, `lift.rs`, `colorize.rs`): Modify pixel values post-sampling.
4. **Blur/Convolution Effects** (`gaussian_blur.rs`, `lens_blur.rs`, `sharpen.rs`): Run 2-pass horizontal/vertical convolution filters on the overall rendered layer canvas.
5. **Keying** (`luma_key.rs`): Controls layer transparency based on pixel luminance.

All pixel effects are dispatched in serial sequence via `apply_pixel_effects` in `packages/renderer-core/src/effects/mod.rs`.

## Key Design Principles

- **Deterministic Evaluation**: Given the same project definition and time offset, `evaluate` always produces the exact same resolved properties. 
- **Atomic Database Updates**: All layer updates in `preview-service` are wrapped in SQLite database transactions to preserve history integrity.
- **Rayon Parallelism**: Multi-threaded row-based render loops speed up pixel-processing effects, and multi-threaded frame loops parallelize offline exporting.
- **WebP WebSocket Transport**: Using raw binary WebP-encoded buffers drastically decreases networking overhead during active browser previews.
