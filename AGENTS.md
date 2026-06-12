# Alight Motion Renderer — Coordinate System & Conventions

## Coordinate Space

Alight Motion XML uses a **mixed coordinate system** where different properties live in different coordinate spaces:

| Property | Coordinate space | Example (1080×1920 canvas) |
|---|---|---|
| `location` | **Pixels** (matches canvas) | `540, 960` = canvas center |
| `size` | **Logical points** (half-canvas) | `540, 960` → `1080, 1920` px |
| `scale` | **Unitless multiplier** | `1.0, 1.0` = no change |
| Effect pixel params | **Logical points** | `offset=300` → `600` px |

The fixed `coord_scale = 2.0` converts logical points to pixel values. It is applied to `size` and pixel-valued effect parameters, but **never** to `location` or `scale`.

### Scene Dimensions

The XML `<scene>` tag has two sets of dimensions:

| Attribute | Purpose |
|---|---|
| `width` / `height` | Internal canvas / coordinate space used for rendering |
| `exportWidth` / `exportHeight` | Target output resolution (may differ from canvas) |

For most projects `width == exportWidth`, but some projects use a smaller canvas with a larger export (e.g., `width=720 exportWidth=1080` = 1.5× scale).

The renderer always renders at `width × height`. To produce the final output at `exportWidth × exportHeight`, the rendered image would need to be upscaled (not yet implemented).

## Rendering Pipeline

### 1. XML → Model (`main.rs`)

```
XML <scene> width/height  →  project.width/height     (pixels, no scaling)
XML <location>            →  layer.location            (pixels, no scaling)
XML <scale>               →  layer.scale               (unitless, no scaling)
XML <property name="size"> → layer.size                (× coord_scale = pixels)
XML <effect> params       →  effect.*                  (× coord_scale where applicable)
```

### 2. Model → Resolved (`eval/timeline.rs`)

Animated properties are evaluated at the current time. Values pass through unchanged — no coordinate transforms.

### 3. Resolved → Canvas (`src/render/compositor/`)

#### Layer source buffer creation (`create_layer_source`)

For layers **with effects** (non-Media fill):
```
buffer_w = layer.size[0] × |layer.scale[0]|
buffer_h = layer.size[1] × |layer.scale[1]|
```
This creates a buffer at the layer's full canvas footprint so effects operate at final resolution.

For **Media** layers or layers without effects: buffer uses raw `layer.size` (the image is loaded at native resolution).

#### Lift (Copy Background) sampling

Uses the forward transform to map each buffer pixel to a canvas coordinate, sampling the composition canvas already rendered below. Key values:
- `half_w = layer.size[0] / 2.0` — center-origin offset
- `dx_per_px = layer.size[0] / buffer_w` — pixel-to-local step

#### Inverse transform compositing (`render_layer`)

For each canvas pixel, the inverse transform maps to layer-local coordinates (center-origin), then adds `half_w`/`half_h` to get buffer coordinates (top-left-origin). The default "stretch" fill mode maps buffer coordinates to source image coordinates:
```
sx = lx_raw / layer.size[0] × src_w
sy = ly_raw / layer.size[1] × src_h
```

## `coord_scale` Application Table

**Scaled (logical → pixels):**

| Property | Where | Reason |
|---|---|---|
| `size` | `main.rs:462` | Half-canvas coords in XML |
| `offset` (Offset effect) | `main.rs:838` | Pixel displacement |
| `stretch`, `offset` (StretchSegment) | `main.rs:850-851` | Pixel stretch/offset |
| `radius` (GaussianBlur) | `main.rs:813` | Pixel blur radius |
| `radius` (Sharpen) | `main.rs:809` | Pixel blur radius |
| `radius` (LensBlur) | `main.rs:816` | Pixel blur radius |

**NOT scaled (already correct units):**

| Property | Reason |
|---|---|
| `location` | Already in pixel coordinates |
| `scale` | Unitless multiplier |
| Vignette params (`scale`, `feather`, etc.) | Normalized 0–1 range |
| Exposure params (`exposure`, `gamma`) | Exposure values |
| Saturation/Vibrance | Normalized range |
| Blend modes, opacities, colors | Unitless |

## Debug Layout Mode

When `--debug-layout` is active:

1. Canvas is expanded to `2 × scene.width × scene.height`
2. Layer transforms (location, scale) are halved relative to canvas center
3. Bounding boxes and labels are drawn around each layer
4. The project's canvas boundary is outlined in the center quadrant

This mode visualizes the coordinate space but does **not** change effect processing (effects still operate on full-size layer buffers).

## Future Considerations

If full `exportWidth × exportHeight` output support is added, the render pipeline will need:

1. Compute `output_scale_x = exportWidth / width` and `output_scale_y = exportHeight / height`
2. Upscale the final rendered `width × height` buffer to `exportWidth × exportHeight`
3. Effect `coord_scale` would remain `2.0` since effects operate on internal-resolution buffers
