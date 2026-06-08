# Effects Catalog

## Transform Modifiers (evaluated in `eval::effects`)

| Effect | XML Name | Params | Behavior |
|--------|----------|--------|----------|
| **Oscillate** | `oscillate` | axis, frequency, amplitude, phase, waveform | Sinusoidal position displacement along selected axis |
| **Swing** | `swing` | frequency, amplitude, decay | Pendulum rotation oscillation with optional decay |
| **RandomDisplace** | `randomDisplace` | speed, magnitude, seed | Deterministic hash-based noise displacement (reproducible) |

## Temporal Effects (affect layer visibility/opacity)

| Effect | XML Name | Params | Behavior |
|--------|----------|--------|----------|
| **MotionBlur** | `motionBlur` | samples, shutter angle, shutter phase | Temporal supersampling: evaluate multiple sub-frame samples and average them |
| **Blink** | `blink` | frequency, phase | Periodic on/off visibility |
| **Fade** | `fade` | fade_in, fade_out | Linear opacity fade at layer start/end boundaries |

## UV Effects (modify sampling coordinates)

| Effect | XML Name | Params | Behavior |
|--------|----------|--------|----------|
| **Tile** | `tile` | scale, phase, vert_offset, mirror, angle | Repeating/mirror tiling with brick stagger and per-tile rotation |
| **Offset** | `offset` | shift_x, shift_y, wrap | Scroll/shift UV coordinates with optional wrapping |
| **StretchSegment** | `stretchSegment` | segment_start, segment_end, stretch_factor | Split frame and stretch a contiguous segment |

## Color/Compositing Effects

| Effect | XML Name | Params | Behavior |
|--------|----------|--------|----------|
| **Exposure** | `exposure` | ev | Exposure adjustment in EV stops (photographic) |
| **BrightnessContrast** | `brightnessContrast` | brightness, contrast, contrast_pivot | Brightness/contrast with configurable pivot point |
| **SaturationVibrance** | `saturationVibrance` | saturation, vibrance | HSL saturation and vibrance adjustment |
| **ColorTint** | `colorTint` | color, intensity, blend_mode | Tint overlay with configurable blend mode |
| **HighlightShadow** | `highlightShadow` | highlights, shadows, highlight_radius, shadow_radius | Perceptual highlight/shadow adjustment |
| **Vignette** | `vignette` | center_x, center_y, feather, radius, color, opacity | Radial darkening at frame edges |
| **ColorFill** | `colorFill` | color, blend_mode, opacity | Solid color fill composited with blend mode |
| **FindEdges** | `findEdges` | threshold, color, opacity | Sobel edge detection overlay |
| **Sharpen** | `sharpen` | amount, radius | Unsharp mask sharpening |
| **GaussianBlur** | `gaussianBlur` | radius, sigma | Separable 2-pass Gaussian blur |
| **LensBlur** | `lensBlur` | radius, quality, brightness | Simulated lens/bloom blur |
| **GradientOverlay** | `gradientOverlay` | gradient, angle, style, blend_mode, opacity | Gradient fill composited over layer |
| **Lift (Copy Background)** | `lift` | — | Copies background pixels at corresponding positions with optional offset (used for chromatic aberration / glitch effects) |

## Keying Effects

| Effect | XML Name | Params | Behavior |
|--------|----------|--------|----------|
| **LumaKey** | `lumaKey` | threshold, tolerance, edge_feather | Alpha mask based on luminance values |

## Notes

- Effects are applied in the order they appear in the layer's effect stack
- Transform modifiers (oscillate, swing, randomDisplace) are applied during the `eval` phase and modify the layer's transform before rendering
- All other effects are applied during the `render` phase as per-pixel or per-image post-processing
- Blur effects (GaussianBlur, LensBlur) operate on the entire rendered layer image, not per-pixel
- MotionBlur is listed as temporal but is currently a stub — it requires multi-sample evaluation
- The `Unknown` variant exists for unrecognized XML effect IDs, ensuring forward compatibility
