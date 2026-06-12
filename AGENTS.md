# am-renderer — Agent Guide

## Quick start
```bash
cargo build                          # Debug build
cargo build --release                # Release build (LTO, thin LTO, panic=abort)
cargo test                           # All tests (inline only — parser/xml.rs, effects/transform.rs)
cargo test parser                    # Parser tests only
cargo test effects                   # Effects tests only
cargo run -- info -i presets/preset1.xml
cargo run -- render -i presets/preset1.xml -a assets -o output.mp4
cargo run --example analyze_presets  # Inline example
cargo run --example auto_pair -- -i presets/preset1.xml -s <src> -o assets
RUST_LOG=debug cargo run -- render ...  # env_logger debug output
```
- **Dep**: `ffmpeg` on PATH (required only for MP4 export). No other system deps.
- **Rust edition 2021**, no nightly features.

## Project structure
```
src/
  lib.rs          — Re-exports 6 modules
  main.rs         — CLI entrypoint (clap). Defines Render + Info subcommands.
  config.rs       — TOML config → Vec<disabled_effect_names> for filtering effects
  parser/         — XML deserialization (quick-xml) + converter to domain model
    types.rs      — Raw XML serde types (XmlScene, XmlShape, etc.)
    xml.rs        — parse_xml() entry, contains only #[cfg(test)] unit tests
    converter.rs  — convert_project(): XmlScene → Project with Animated<T> fields
  model/          — Domain types: Project, Layer, Effect, Animated<T>, Keyframe
    animation.rs  — Animated<T>::evaluate(normalized_t), cubic bezier via Newton-Raphson
    effect.rs     — EffectType enum with 25+ variants, all parameter structs
  eval/           — Project → ResolvedScene at time t
    timeline.rs   — evaluate(project, time_secs) — stateless, deterministic
    transform.rs  — build_transform_matrix, invert_transform, transform_point
  render/         — All rendering + effects
    compositor/   — render_scene(), render_layer(), create_layer_source()
    effects/      — 20+ effect files, dispatched via apply_pixel_effects() in mod.rs
    blending.rs   — Porter-Duff per-pixel blend modes
    debug_layout.rs / debug_effects.rs
  export/         — PNG sequence (export_sequence) + MP4 via ffmpeg (export_mp4)
examples/         — analyze_presets, auto_pair, test_sizing
presets/          — 9 XML test presets
```

## Coordinate system (`coord_scale = 2.0`)

Alight Motion XML mixes coordinate spaces. The fixed `coord_scale = 2.0` converts logical points → pixels. Applied in `parser/converter.rs`.

| Property | Space | Scaled? | Where |
|---|---|---|---|
| `location` | Pixels (canvas coords) | **No** | — |
| `size` | Logical points (half-canvas) | **Yes** (×2) | `converter.rs:158` |
| `scale` | Unitless multiplier | **No** | — |
| `radius` (GaussianBlur/Sharpen/LensBlur) | Logical points | **Yes** (×2) | `converter.rs:507,511,514` |

**NOT scaled** (already correct): `location`, `scale`, Vignette params (normalized 0–1), Exposure params, Saturation/Vibrance, Offset vector, StretchSegment stretch/offset, Swirl radius, blend modes, opacities, colors

### Scene dimensions
- XML `<scene width="W" height="H" exportWidth="EW" exportHeight="EH">`
- `width`/`height` = internal canvas (what the renderer uses)
- `exportWidth`/`exportHeight` = output resolution (may differ — upscale not yet implemented)

## Key architecture — what to know

1. **Stateless evaluation**: `evaluate(time_secs)` is deterministic. No frame-to-frame state. Given same `Project` + time → same `ResolvedScene`. Temporal effects (motion blur, blink) must use explicit multi-sample, not state accumulation.

2. **Inverse-transform sampling**: For each output pixel, compute inverse transform to find source UV. Sub-pixel accurate. Two-pass separable blurs. Per-row parallelism via rayon.

3. **Compositing**: Layers rendered bottom-to-top onto a transparent composition canvas (so Lift effects sample only other layers, not project background), then blended onto main canvas via Porter-Duff "over".

4. **Effect dispatch**: `apply_pixel_effects()` in `render/effects/mod.rs` matches `EffectType` variants to effect functions. Transform-modifying effects (Oscillate, Swing, RandomDisplace, Spin) are handled separately in `transform.rs` before pixel effects run.

5. **Coord scaling is applied in `parser/converter.rs` during XML→model conversion**, not in effects code. See `coord_scale` application table above.

## How to add a new effect
1. **Parameter struct** → `model/effect.rs` — add `EffectType` variant + params struct with `Default`
2. **XML parsing** → `parser/types.rs` if new XML tags needed; `parser/converter.rs` in `convert_effect()`
3. **If transform modifier**: implement in `render/effects/transform.rs`
4. **If pixel/image effect**: new file in `render/effects/` + match arm in `apply_pixel_effects()` in `mod.rs`
5. **Add type_name()** to `EffectType::type_name()` in `effect.rs` (needed for config-based disabling)

## CLI flags worth knowing
- `--auto-pair` — Round-robin assign asset files to XML virtual URIs (useful when media was renamed/moved)
- `--config` — TOML file listing `disabled_effects = ["GaussianBlur", "Vignette"]` to skip effects
- `--debug-layout` — Expanded canvas with bounding boxes, layer labels, dimmed out-of-bounds areas
- `--debug-effects` — Render each effect in isolation for debugging
- `--dump-graph` — Print resolved layer/effect tree (useful for debugging parse issues)
- `--frame N` vs `--start-frame N / --end-frame N / --start-time S / --end-time S` — Cannot combine `--frame` with range flags; cannot combine `--start-frame` with `--start-time`

## Testing quirks
- Tests are **inline only**: `#[cfg(test)] mod tests` in `parser/xml.rs` and `render/effects/transform.rs`
- No integration tests, no fixtures directory, no benchmarks
- `cargo test` is fast — no external services needed

## Code conventions
- 4-space indent, `mod.rs` pattern
- `Animated<T>` for animatable fields; `Lerp` trait must be implemented for any type used as `Animated<T>` (already done: f32, [f32;2], [f32;3], Vec2)
- `anyhow::Result` throughout; avoid unwrap/expect in library code
- Image: `RgbaImage` (u8 storage), `[f32; 4]` for processing
- Math: `glam::Vec2`, `glam::Mat3` for 2D affine
