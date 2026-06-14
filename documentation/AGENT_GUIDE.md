# AI Agent Guide

This document provides structured context for AI agents working with the `am-renderer` Cargo workspace.

## Quick Facts

- **Language**: Rust (edition 2021)
- **Crate Layout**: Cargo Workspace
  - `graph-resolver` — Scene model (`Project`, `Layer`, `Effect`, `Animated<T>`) + evaluator.
  - `alight-parser` — Converts raw XML to domain models, applying coordinate system scaling.
  - `renderer-core` — Compositing, blend modes, and separable pixel/transform effects.
  - `export-service` — PNG/MP4 sequence export.
  - `preview-service` — Axum REST + WebSocket server with stateful SQLite database (tracks mutations, undo/redo).
  - `web-editor` — React + TS timeline editor & preview UI.
  - `integration-tests` — Black-box integration tests.
- **Entry points**: 
  - CLI: `src/main.rs` (root crate)
  - Preview Server: `packages/preview-service/src/main.rs`
  - React Web Editor: `packages/web-editor/src/main.rs`
- **External dep**: `ffmpeg` (required for MP4 export only)

## Pipeline (Data Flow)

```
XML File (Alight Motion XML)
  │
  ▼ [alight-parser]
Project / Layer / Effect (Domain model with Animated<T>)
  │
  ▼ [graph-resolver::timeline::evaluate(t)]
ResolvedScene (Concrete values at time t)
  │
  ▼ [renderer-core::compositor::render_scene()]
RgbaImage (Rendered frame)
  ├──► [export-service] ──────► PNG / MP4 video files
  └──► [preview-service] ────► WebP compressed stream over WebSocket
```

## Coordinate System (`coord_scale = 2.0`)

Alight Motion XML mixes coordinate spaces. The fixed `coord_scale = 2.0` converts logical points → pixels. Applied in `packages/alight-parser/src/converter.rs`.

| Property | Space | Scaled? | Where |
|---|---|---|---|
| `location` | Pixels (canvas coords) | **No** | — |
| `size` | Logical points (half-canvas) | **Yes** (×2) | `converter.rs` |
| `scale` | Unitless multiplier | **No** | — |
| `radius` (GaussianBlur/Sharpen/LensBlur) | Logical points | **Yes** (×2) | `converter.rs` |

**NOT scaled**: `location`, `scale`, Vignette params (normalized 0–1), Exposure params, Saturation/Vibrance, Offset vector, StretchSegment stretch/offset, Swirl radius, blend modes, opacities, colors.

## Where to Add New Effects

1. **Define parameter struct & variant** in `packages/graph-resolver/src/model/effect.rs` — add variant to `EffectType` enum and create a params struct.
2. **Add XML-to-model conversion** in `packages/alight-parser/src/converter.rs` under `convert_effect()`.
3. **Implement effect behavior**:
   - **Transform Modifiers**: Implement in `packages/renderer-core/src/effects/transform.rs`.
   - **Pixel/Image Effects**: Create a new file in `packages/renderer-core/src/effects/` (e.g., `my_effect.rs`) and add a match arm to `apply_pixel_effects()` in `packages/renderer-core/src/effects/mod.rs`.
4. **Update DB serializations**: Ensure any new optional fields are deserializable in SQLite endpoints or database payloads.

## State Management & Mutations

The `preview-service` tracks stateful projects inside SQLite (`db.sqlite`):
- REST APIs are defined in `packages/preview-service/src/main.rs`.
- DB mutations are implemented in `packages/preview-service/src/mutations.rs` and `packages/preview-service/src/db.rs`.
- Mutations push undo/redo snapshots to an in-memory stack.
- To prevent serialization errors, make sure JSON requests for mutations supply all fields (including `effects`, `fill_image`, `gradient`, `media_fill_mode`, `s`), since structs might not have `#[serde(default)]`.

## Build, Test & Run Commands

```bash
# Clean compilation check
cargo check

# Run Unit Tests
cargo test --workspace --exclude integration-tests

# Start the Axum Web Server (Local port 8080)
cargo run --release -p preview-service

# Run Frontend Editor (Vite Mode, local port 5173)
cd packages/web-editor
npm install
npm run dev

# Run Integration/E2E tests (Requires a running preview-service server)
# CRITICAL: Always use --test-threads=1 to run sequentially!
cargo test -p integration-tests -- --test-threads=1
```

## Testing & WebSocket Caveats

1. **Sequential Integration Testing**: E2E tests interact with a single global WebSocket channel and SQLite backend database. Running them concurrently causes socket drop (`AlreadyClosed`) or database locks. Always run using `--test-threads=1`.
2. **Scheduler Clamp**: The `PlaybackScheduler` clamps FPS to `[0.1, 240.0]` in `packages/preview-service/src/scheduler.rs` to prevent zero or negative time step division panics.
3. **Active Render Cancellations**: When a playback is paused or WebSocket connection is dropped, `PlaybackScheduler::pause()` aborts all ongoing rendering tasks.

## Code Style

- 4-space indentation.
- `snake_case` for functions/variables, `CamelCase` for types/enums.
- Doc comments (`///` and `//!`) on all public items.
- Avoid unwrap/expect in library code; use `anyhow` context instead.
