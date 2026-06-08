# Architecture

## Overview

`am-renderer` is a headless CPU-based renderer for Alight Motion XML projects. It parses `.xml` project files exported from Alight Motion, evaluates animated properties at arbitrary timestamps, and renders frames as PNG sequences or MP4 video.

## Pipeline

```
XML file  --[parser]--> XmlScene (raw serde types)
                             |
                    [main.rs: convert_project]
                             |
              Project (domain model with Animated<T>)
                             |
              [eval::timeline::evaluate(time_secs)]
                             |
              ResolvedScene (concrete values at t)
                             |
              [render::compositor::render_scene()]
                             |
              RgbaImage (rendered frame)
                             |
              [export::png or export::video]
                             |
              PNG Sequence or MP4
```

## Core Principle

`evaluate(time)` is stateless and deterministic. Given the same `Project` and time, it always produces the same `ResolvedScene`. No frame-to-frame hidden state, no temporal caches. This means motion blur, blink, and other temporal effects must be implemented via explicit multi-sample evaluation at the render level, not via state accumulation.

## Layer Model

Each layer in the XML is converted to a `Layer` struct containing:
- An animated `LayerTransform` (anchor, position, scale, rotation, opacity)
- A `FillType` (None, Media, Color, or Gradient)
- A list of `Effect` instances
- Timing metadata (start time, duration, visibility)

During evaluation, all animated properties are resolved to concrete values at the given timestamp. The resolved layer is then rendered bottom-to-top using inverse-transform sampling with per-pixel blend modes.

## Effect Classification

Effects are classified into categories based on where they apply in the rendering pipeline:

1. **Transform Modifiers** (eval layer) - Modify the layer's transform matrix before rendering: Oscillate, Swing, RandomDisplace
2. **Temporal Effects** (render layer) - Affect visibility/opacity over time: MotionBlur, Blink, Fade
3. **UV Effects** (render layer) - Modify sampling coordinates: Tile, Offset, StretchSegment
4. **Color Effects** (render layer) - Modify pixel colors post-sampling: Exposure, BrightnessContrast, HSL, ColorTint, Vignette, Sharpen, FindEdges, GaussianBlur, LensBlur, GradientOverlay, ColorFill, Lift (Copy Background)
5. **Keying** (render layer) - Alpha manipulation: LumaKey

## Key Design Decisions

- **No GPU**: Entirely CPU-based software rasterization using the `image` crate
- **Inverse-transform sampling**: For each output pixel, compute the inverse transform to find the source pixel, enabling correct sub-pixel transforms
- **Porter-Duff "over" compositing**: Layers are composited bottom-to-top with alpha-aware blending
- **Separable blurs**: Gaussian and box blur implemented as two-pass (horizontal + vertical) for O(n) performance
- **Newton's method for cubic bezier**: Keyframe easing uses Newton-Raphson to solve for t given x, with fallback to bisection
- **Deterministic noise**: RandomDisplace uses a hash-based deterministic noise function so frames are reproducible
