# Effects Catalog

## Transform Modifiers (evaluated in `renderer-core/src/effects/transform.rs`)

| Effect | XML Name | Params | Behavior |
|--------|----------|--------|----------|
| **Oscillate** | `oscillate` | axis, frequency, amplitude, phase, waveform | Sinusoidal position/scale displacement along selected axis |
| **Swing** | `swing` | frequency, amplitude, decay | Pendulum rotation oscillation with optional decay |
| **RandomDisplace** | `randomDisplace` | speed, magnitude, seed | Deterministic hash-based noise displacement (reproducible) |
| **Spin** | `spin` | speed, pivot | Constant auto-rotation around specified center/pivot |

## Temporal Effects (affect layer visibility/opacity)

| Effect | XML Name | Params | Behavior |
|--------|----------|--------|----------|
| **MotionBlur** | `motionBlur` | samples, shutter angle, shutter phase | Temporal supersampling: evaluate multiple sub-frame samples and average them (stub) |
| **Blink** | `blink` | frequency, phase | Periodic on/off visibility (stub) |
| **Fade** | `fade` | fade_in, fade_out | Linear opacity fade at layer start/end boundaries |

## UV Effects (modify sampling coordinates)

| Effect | XML Name | Params | Behavior |
|--------|----------|--------|----------|
| **Tile** | `tile` | scale, phase, vert_offset, mirror, angle | Repeating/mirror tiling with brick stagger and per-tile rotation |
| **Offset** | `offset` | shift_x, shift_y, wrap | Scroll/shift UV coordinates with optional wrapping |
| **StretchSegment** | `stretchSegment` | segment_start, segment_end, stretch_factor | Split frame and stretch a contiguous segment |
| **Swirl** | `swirl` | radius, strength, center | Dynamic vortex distortion warping coordinates inward |
| **Wipe** | `wipe` | progress, angle, feather | Linear transition overlay masking out portions of the texture |

## Color/Compositing Effects

| Effect | XML Name | Params | Behavior |
|--------|----------|--------|----------|
| **Exposure** | `exposure` | ev | Exposure adjustment in EV stops (photographic) |
| **BrightnessContrast** | `brightnessContrast` | brightness, contrast, contrast_pivot | Brightness/contrast with configurable pivot point |
| **SaturationVibrance** | `saturationVibrance` | saturation, vibrance | HSL saturation and vibrance adjustment |
| **ColorTint** | `colorTint` | color, intensity, blend_mode | Tint overlay with configurable blend mode |
| **HighlightShadow** | `highlightShadow` | highlights, shadows, highlight_radius, shadow_radius | Perceptual highlight/shadow adjustment (stub) |
| **Vignette** | `vignette` | center_x, center_y, feather, radius, color, opacity | Radial darkening at frame edges |
| **Colorize** | `colorize` | hue, saturation, light | Preserves source luminance while shifting tint and saturation |
| **FindEdges** | `findEdges` | threshold, color, opacity | Sobel edge detection overlay |
| **Sharpen** | `sharpen` | amount, radius | Unsharp mask sharpening (stub) |
| **GaussianBlur** | `gaussianBlur` | radius, sigma | Separable 2-pass Gaussian blur |
| **LensBlur** | `lensBlur` | radius, quality, brightness | Simulated lens/bokeh blur (stub) |
| **GradientOverlay** | `gradientOverlay` | gradient, angle, style, blend_mode, opacity | Gradient fill overlay (stub) |
| **Lift (Copy Background)** | `lift` | — | Copies background pixels at corresponding positions (used for chromatic aberration / glitch effects) |

## Keying Effects

| Effect | XML Name | Params | Behavior |
|--------|----------|--------|----------|
| **LumaKey** | `lumaKey` | threshold, tolerance, edge_feather | Alpha mask based on luminance values (stub) |

## Notes

- Effects are applied in the order they appear in the layer's effect stack.
- Transform modifiers (oscillate, swing, randomDisplace, spin) are applied via `renderer-core/src/effects/transform.rs` and modify the layer's transform before compositing.
- All other pixel/UV effects are applied during the rendering phase via the centralized `renderer-core/src/effects/mod.rs::apply_pixel_effects()` dispatcher.
- Blur effects (GaussianBlur, LensBlur) operate on the entire rendered layer image rather than individual pixels.
- The `Unknown` variant is used for unrecognized XML effect IDs, ensuring forward compatibility.
