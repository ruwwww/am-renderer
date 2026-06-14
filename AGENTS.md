# am-renderer — Workspace & Agent Guide

## Workspace Crates
The project is organized as a Cargo workspace with the following packages:
- **`graph-resolver`**: Core scene evaluation and animated interpolation.
- **`alight-parser`**: Deserialization of Alight Motion XML scenes to model definitions.
- **`renderer-core`**: Separable effects, layouts, and Porter-Duff layer composition.
- **`preview-service`**: Stateful Axum REST API and WebSocket preview server backed by SQLite (with transactional mutations, undo/redo). Serves the editor static bundle at root.
- **`web-editor`**: React + TypeScript frontend editor UI.
- **`integration-tests`**: Black-box integration and workload tests.

## Quick start
```bash
# Core CLI Commands
cargo build                          # Debug build
cargo build --release                # Release build (LTO, thin LTO, panic=abort)
cargo test --workspace --exclude integration-tests  # Unit tests

# Run Web Editor Backend Server
cargo run --release -p preview-service  # Starts server on http://localhost:8080/

# Run Web Editor React Frontend (Vite Dev Mode)
cd packages/web-editor
npm install
npm run dev                          # Starts dev server on http://localhost:5173/

# Run Integration & E2E Tests (requires running preview-service server on port 8080)
cargo test -p integration-tests -- --test-threads=1
```
- **Dep**: `ffmpeg` on PATH (required only for MP4 export). No other system deps.
- **Rust edition 2021**, no nightly features.

## Project structure
```
packages/
  graph-resolver/     — Domain model (Project, Layer, Effect, Animated<T>) + Evaluator
  alight-parser/      — Converts raw XML parsed scenes to graph-resolver domain model
  renderer-core/      — Composition rendering, blend modes, and separable pixel effects
  preview-service/    — SQLite DB schemas, API endpoints, playback scheduler, WS handler
  web-editor/         — React frontend timeline editor, canvas viewer, websocket sync
  integration-tests/  — E2E integration, websocket, rest, workload, and DB tests
presets/              — 10 XML test presets
```

## Coordinate system (`coord_scale = 2.0`)

Alight Motion XML mixes coordinate spaces. The fixed `coord_scale = 2.0` converts logical points → pixels. Applied in `packages/alight-parser/src/converter.rs`.

| Property | Space | Scaled? | Where |
|---|---|---|---|
| `location` | Pixels (canvas coords) | **No** | — |
| `size` | Logical points (half-canvas) | **Yes** (×2) | `converter.rs` |
| `scale` | Unitless multiplier | **No** | — |
| `radius` (GaussianBlur/Sharpen/LensBlur) | Logical points | **Yes** (×2) | `converter.rs` |

**NOT scaled** (already correct): `location`, `scale`, Vignette params (normalized 0–1), Exposure params, Saturation/Vibrance, Offset vector, StretchSegment stretch/offset, Swirl radius, blend modes, opacities, colors

### Scene dimensions
- XML `<scene width="W" height="H" exportWidth="EW" exportHeight="EH">`
- `width`/`height` = internal canvas (what the renderer uses)
- `exportWidth`/`exportHeight` = output resolution (may differ — upscale not yet implemented)

## Key architecture — what to know

1. **Stateless evaluation**: `evaluate(time_secs)` is deterministic. No frame-to-frame state. Given same `Project` + time → same `ResolvedScene`. Temporal effects (motion blur, blink) must use explicit multi-sample, not state accumulation.

2. **Inverse-transform sampling**: For each output pixel, compute inverse transform to find source UV. Sub-pixel accurate. Two-pass separable blurs. Per-row parallelism via rayon.

3. **Compositing**: Layers rendered bottom-to-top onto a transparent composition canvas (so Lift effects sample only other layers, not project background), then blended onto main canvas via Porter-Duff "over".

4. **Effect dispatch**: `apply_pixel_effects()` in `packages/renderer-core/src/render/effects/mod.rs` matches `EffectType` variants to effect functions. Transform-modifying effects (Oscillate, Swing, RandomDisplace, Spin) are handled separately in `transform.rs` before pixel effects run.

5. **Coord scaling is applied during XML→model conversion**, not in effects code. See `coord_scale` application table above.

6. **Web Socket Frame Streaming & Scheduler**:
   - The websocket client connects and receives a binary stream of WebP compressed frames.
   - When a WebSocket client connects, the frame stream channel is registered to the `PlaybackScheduler`.
   - On scheduler pause or connection drop, ongoing render tasks are cancelled. The server claps FPS to `[0.1, 240.0]` to prevent zero/negative duration crashes.

7. **Transactional Undo/Redo & SQLite**:
   - Layer property mutations are written transactionally to SQLite.
   - Undo/redo stacks are managed in memory at endpoint level (`/api/projects/:id/undo` and `/api/projects/:id/redo`).
   - Project snapshots are serialized and swapped in SQLite during undo/redo actions.

## Testing guidelines
- **Unit Tests**: Run unit tests with `cargo test --workspace --exclude integration-tests`.
- **Integration Tests**: Integrations tests reside under `packages/integration-tests/tests`.
  - Always run integration tests **sequentially** using `--test-threads=1` because they interact with the single global state of the running backend server.
  ```bash
  cargo test -p integration-tests -- --test-threads=1
  ```

## Code conventions
- 4-space indent, `mod.rs` pattern.
- `Animated<T>` for animatable fields; `Lerp` trait must be implemented for any type used as `Animated<T>` (already done: f32, [f32;2], [f32;3], Vec2).
- `anyhow::Result` throughout; avoid unwrap/expect in library code.
- Image: `RgbaImage` (u8 storage), `[f32; 4]` for processing.
- Math: `glam::Vec2`, `glam::Mat3` for 2D affine.
