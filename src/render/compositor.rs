//! Main software compositor — renders resolved scenes to RGBA images.
//!
//! Uses inverse-transform sampling: for each canvas pixel, compute where it
//! maps in the source layer, sample the source, then blend onto the canvas.
//!
//! Performance notes:
//!  - Per-row parallelism via rayon (all CPU cores)
//!  - Lift (Copy Background) uses affine stepping — no per-pixel matrix multiply
//!  - blend_pixel uses integer fixed-point arithmetic
//!  - Effect LUTs precomputed once per layer

use crate::eval::timeline::ResolvedScene;
use crate::eval::transform::{build_transform_matrix, invert_transform, transform_point};
use crate::model::{ResolvedLayer, FillType, Gradient};
use crate::render::blending::blend_pixel;
use crate::render::effects::transform::apply_transform_effects;
use crate::render::effects::lift::apply_lift;
use image::{RgbaImage, Rgba};
use anyhow::{Context, Result, bail};
use std::path::Path;
use std::collections::HashMap;
use log::{debug, warn};
use rayon::prelude::*;

use std::sync::Arc;

/// Cache for loaded source images to avoid re-reading from disk.
#[derive(Clone)]
pub struct ImageCache {
    images: HashMap<String, Arc<RgbaImage>>,
    pub virtual_mappings: HashMap<String, std::path::PathBuf>,
}

impl ImageCache {
    /// Create a new empty image cache.
    pub fn new() -> Self {
        Self {
            images: HashMap::new(),
            virtual_mappings: HashMap::new(),
        }
    }

    /// Create a new cache pre-populated with virtual mappings.
    pub fn new_with_mappings(mappings: HashMap<String, std::path::PathBuf>) -> Self {
        Self {
            images: HashMap::new(),
            virtual_mappings: mappings,
        }
    }

    /// Set virtual mappings for media URIs to physical files.
    pub fn set_virtual_mappings(&mut self, mappings: HashMap<String, std::path::PathBuf>) {
        self.virtual_mappings = mappings;
    }

    /// Clone just the virtual mappings (cheap — no image data).
    pub fn virtual_mappings_clone(&self) -> HashMap<String, std::path::PathBuf> {
        self.virtual_mappings.clone()
    }

    /// Load an image by URI, returning a reference to the cached image.
    ///
    /// If the image has already been loaded, returns the cached version.
    /// Otherwise, resolves the URI to a file path within `assets_dir` and loads it.
    pub fn load(&mut self, uri: &str, assets_dir: &Path) -> Result<Arc<RgbaImage>> {
        if !self.images.contains_key(uri) {
            let img = if let Some(physical_path) = self.virtual_mappings.get(uri) {
                image::open(physical_path)
                    .with_context(|| format!("Failed to open virtually paired image: {}", physical_path.display()))?
                    .to_rgba8()
            } else {
                load_image_from_uri(uri, assets_dir)?
            };
            self.images.insert(uri.to_string(), Arc::new(img));
        }
        Ok(Arc::clone(self.images.get(uri).unwrap()))
    }
}


/// Render a resolved scene to an RGBA image.
///
/// Creates a canvas filled with the background color, then composites
/// each layer bottom-to-top.
///
/// # Arguments
/// * `scene` - The resolved scene to render
/// * `image_cache` - Cache for loaded source images
/// * `assets_dir` - Directory containing media assets
///
/// # Returns
/// The rendered RGBA image.
pub fn render_scene(
    scene: &ResolvedScene,
    image_cache: &mut ImageCache,
    assets_dir: &Path,
    debug_layout: bool,
    disabled_effects: &[String],
) -> Result<RgbaImage> {
    let (canvas_w, canvas_h) = if debug_layout {
        (scene.width * 2, scene.height * 2)
    } else {
        (scene.width, scene.height)
    };
    let mut canvas = RgbaImage::new(canvas_w, canvas_h);

    if debug_layout {
        // Fill outer background with dark gray
        for pixel in canvas.pixels_mut() {
            *pixel = Rgba([26, 26, 26, 255]);
        }

        // Draw canvas interior filled with project bg color (at 0.5x scale in center of expanded canvas)
        let proj_w = scene.width as f32;
        let proj_h = scene.height as f32;
        let cx = canvas_w as f32 / 2.0;
        let cy = canvas_h as f32 / 2.0;

        let x0 = (cx - proj_w * 0.25) as u32;
        let x1 = (cx + proj_w * 0.25) as u32;
        let y0 = (cy - proj_h * 0.25) as u32;
        let y1 = (cy + proj_h * 0.25) as u32;

        let project_bg = to_rgba_u8(scene.bg_color);
        for y in y0..y1 {
            for x in x0..x1 {
                if x < canvas.width() && y < canvas.height() {
                    canvas.put_pixel(x, y, project_bg);
                }
            }
        }

        // Canvas frame boundary outline and label will be drawn on top of layers at the end of rendering
    } else {
        // Fill with background color
        let bg = to_rgba_u8(scene.bg_color);
        for pixel in canvas.pixels_mut() {
            *pixel = bg;
        }
    }

    // Create a separate, transparent composition canvas for rendering the layers.
    // This ensures that:
    // 1. Layers with Lift (Copy Background) only sample other layers, not the project background color.
    // 2. Blend modes (Multiply, Screen, Subtract, etc.) blend correctly with transparent backgrounds.
    let mut comp_canvas = RgbaImage::new(canvas_w, canvas_h);

    // Composite layers bottom to top onto the composition canvas
    for layer in &scene.layers {
        if let Err(e) = render_layer(&mut comp_canvas, layer, image_cache, assets_dir, scene.width, scene.height, debug_layout, disabled_effects) {
            warn!("Failed to render layer '{}' (id={}): {}",
                  layer.label.as_deref().unwrap_or("unnamed"), layer.id, e);
        }
    }

    // Blend the final composition canvas onto the main canvas
    // (using standard Porter-Duff "over" blending for each pixel)
    for y in 0..canvas_h {
        for x in 0..canvas_w {
            let comp_pixel = *comp_canvas.get_pixel(x, y);
            if comp_pixel[3] > 0 {
                let canvas_pixel = *canvas.get_pixel(x, y);
                // Blend with Normal mode at 1.0 opacity
                let blended = blend_pixel(canvas_pixel, comp_pixel, crate::model::BlendMode::Normal, 1.0);
                canvas.put_pixel(x, y, blended);
            }
        }
    }

    // Second pass: Draw bounding box outlines & labels on top of everything
    if debug_layout {
        let proj_w = scene.width as f32;
        let proj_h = scene.height as f32;
        let cx = canvas_w as f32 / 2.0;
        let cy = canvas_h as f32 / 2.0;

        let x0 = (cx - proj_w * 0.25) as u32;
        let x1 = (cx + proj_w * 0.25) as u32;
        let y0 = (cy - proj_h * 0.25) as u32;
        let y1 = (cy + proj_h * 0.25) as u32;

        // Apply a semi-transparent dark overlay (dimming by 60%) to everything outside the canvas boundaries
        for y in 0..canvas_h {
            for x in 0..canvas_w {
                if x < x0 || x >= x1 || y < y0 || y >= y1 {
                    let pixel = canvas.get_pixel_mut(x, y);
                    pixel[0] = ((pixel[0] as u32 * 100) / 255) as u8;
                    pixel[1] = ((pixel[1] as u32 * 100) / 255) as u8;
                    pixel[2] = ((pixel[2] as u32 * 100) / 255) as u8;
                }
            }
        }

        // Draw light gray outline around the canvas boundaries on top of everything
        draw_rect(&mut canvas, x0 as i32, y0 as i32, (x1 - x0) as i32, (y1 - y0) as i32, Rgba([200, 200, 200, 255]));
        // Label the canvas boundary on top of everything
        draw_text(&mut canvas, x0 as i32 + 10, y0 as i32 + 10, "CANVAS FRAME BOUNDARY", Rgba([200, 200, 200, 255]), 2);

        for layer in &scene.layers {
            draw_layer_debug_outline(&mut canvas, layer, scene.width, scene.height, disabled_effects);
        }
    }

    Ok(canvas)
}

/// Render a single layer onto the canvas.
///
/// Creates a layer buffer, fills it based on fill type (media/color/gradient),
/// applies the layer's transform via inverse sampling, and composites onto
/// the canvas with the layer's blend mode and opacity.
fn render_layer(
    canvas: &mut RgbaImage,
    layer: &ResolvedLayer,
    image_cache: &mut ImageCache,
    assets_dir: &Path,
    scene_w: u32,
    scene_h: u32,
    debug_layout: bool,
    disabled_effects: &[String],
) -> Result<()> {
    // Skip fully transparent layers
    if layer.opacity < 1.0 / 255.0 {
        return Ok(());
    }

    let canvas_w = canvas.width() as f32;
    let canvas_h = canvas.height() as f32;
    let canvas_center = [canvas_w / 2.0, canvas_h / 2.0];

    let layer_w = layer.size[0];
    let layer_h = layer.size[1];

    if layer_w <= 0.0 || layer_h <= 0.0 {
        return Ok(());
    }

    // Apply transform-modifying effects
    let (mut location, mut scale, rotation) = apply_transform_effects(
        &layer.effects,
        layer.location,
        layer.scale,
        layer.rotation,
        layer.time_secs,
        layer.normalized_t,
        disabled_effects,
    );

    if debug_layout {
        let proj_center = [scene_w as f32 / 2.0, scene_h as f32 / 2.0];
        location[0] = (location[0] - proj_center[0]) * 0.5 + canvas_center[0];
        location[1] = (location[1] - proj_center[1]) * 0.5 + canvas_center[1];
        scale[0] *= 0.5;
        scale[1] *= 0.5;
    }

    // Build forward transform: layer-local → canvas coordinates
    let fwd = build_transform_matrix(location, scale, rotation, canvas_center);
    let inv = match invert_transform(&fwd) {
        Some(m) => m,
        None => {
            debug!("Layer '{}' has singular transform, skipping", layer.label.as_deref().unwrap_or("unnamed"));
            return Ok(());
        }
    };

    // Get the source image/buffer for this layer
    let source = create_layer_source(layer, image_cache, assets_dir, canvas, &fwd, disabled_effects)?;
    let src_w = source.width() as f32;
    let src_h = source.height() as f32;

    // Half-size for centering (layer's origin is its center)
    let half_w = layer_w / 2.0;
    let half_h = layer_h / 2.0;

    // --- Precompute per-fill-mode mapping constants (done once, outside loop) ---
    let fill_scale = match layer.media_fill_mode.as_deref() {
        Some("fit")  => (layer_w / src_w).min(layer_h / src_h),
        Some("fill") => (layer_w / src_w).max(layer_h / src_h),
        _            => 1.0,
    };
    let (fill_off_x, fill_off_y) = match layer.media_fill_mode.as_deref() {
        Some("fit")  => ((layer_w - src_w * fill_scale) / 2.0, (layer_h - src_h * fill_scale) / 2.0),
        Some("fill") => (-((src_w * fill_scale - layer_w) / 2.0), -((src_h * fill_scale - layer_h) / 2.0)),
        _            => (0.0, 0.0),
    };
    let media_fill_mode = layer.media_fill_mode.as_deref().unwrap_or("");

    // --- Precompute affine stepping vectors for the inverse transform ---
    // For each canvas row y: local = inv * (0.5 + x, 0.5 + y)
    // We use the affine decomposition: stepping one pixel in X adds a constant delta.
    let inv = &inv;
    let step_x = [inv[0][0], inv[0][1]]; // delta per +1 in canvas-x
    let step_y = [inv[1][0], inv[1][1]]; // delta per +1 in canvas-y
    let origin = transform_point(inv, [0.5_f32, 0.5_f32]); // local at (0,0) canvas

    // For each row, precompute the starting local coords then walk columns with step_x.
    // This replaces the per-pixel matrix multiply with 2 FMAs.

    let canvas_w_u = canvas.width();
    let canvas_h_u = canvas.height();
    let layer_blend_mode = layer.blend_mode;
    let layer_opacity = layer.opacity;

    // Collect rows in parallel; each row produces a Vec of (x, pixel) patches
    let row_patches: Vec<Vec<(u32, Rgba<u8>)>> = (0..canvas_h_u)
        .into_par_iter()
        .map(|cy| {
            let mut patches = Vec::new();
            // Starting local coord for this row at cx=0
            let row_lx0 = origin[0] + step_y[0] * cy as f32;
            let row_ly0 = origin[1] + step_y[1] * cy as f32;

            for cx in 0..canvas_w_u {
                let lx_raw = row_lx0 + step_x[0] * cx as f32 + half_w;
                let ly_raw = row_ly0 + step_x[1] * cx as f32 + half_h;

                if lx_raw < 0.0 || lx_raw >= layer_w || ly_raw < 0.0 || ly_raw >= layer_h {
                    continue;
                }

                let (sx, sy) = match media_fill_mode {
                    "fit" => {
                        let sx_f = (lx_raw - fill_off_x) / fill_scale;
                        let sy_f = (ly_raw - fill_off_y) / fill_scale;
                        if sx_f < 0.0 || sx_f >= src_w || sy_f < 0.0 || sy_f >= src_h {
                            continue;
                        }
                        (sx_f as u32, sy_f as u32)
                    }
                    "fill" => {
                        let sx_f = (lx_raw - fill_off_x) / fill_scale;
                        let sy_f = (ly_raw - fill_off_y) / fill_scale;
                        (
                            (sx_f as u32).min(source.width() - 1),
                            (sy_f as u32).min(source.height() - 1),
                        )
                    }
                    _ => {
                        let sx = (lx_raw / layer_w * src_w) as u32;
                        let sy = (ly_raw / layer_h * src_h) as u32;
                        (
                            sx.min(source.width() - 1),
                            sy.min(source.height() - 1),
                        )
                    }
                };

                let src_pixel = *source.get_pixel(sx, sy);
                if src_pixel[3] == 0 {
                    continue;
                }

                let dst_pixel = *canvas.get_pixel(cx, cy);
                let blended = blend_pixel(dst_pixel, src_pixel, layer_blend_mode, layer_opacity);
                patches.push((cx, blended));
            }
            patches
        })
        .collect();

    // Apply patches back to canvas (sequential, no data races)
    for (cy, patches) in row_patches.into_iter().enumerate() {
        for (cx, pixel) in patches {
            canvas.put_pixel(cx, cy as u32, pixel);
        }
    }

    Ok(())
}

/// Create the base shape image source without any lift (background copy) effect.
fn create_base_shape_source(
    layer: &ResolvedLayer,
    w: u32,
    h: u32,
    image_cache: &mut ImageCache,
    assets_dir: &Path,
) -> Result<RgbaImage> {
    let img = match layer.fill_type {
        FillType::Media => {
            if let Some(ref uri) = layer.fill_image {
                let source = image_cache.load(uri, assets_dir)?;
                (*source).clone()
            } else {
                warn!("Media layer '{}' has no fill image URI", layer.label.as_deref().unwrap_or("unnamed"));
                RgbaImage::new(w, h)
            }
        }
        FillType::Color => {
            let color = to_rgba_u8(layer.fill_color);
            let mut img = RgbaImage::new(w, h);
            for pixel in img.pixels_mut() {
                *pixel = color;
            }
            img
        }
        FillType::Gradient => {
            if let Some(ref gradient) = layer.gradient {
                render_gradient(w, h, gradient)
            } else {
                let color = to_rgba_u8(layer.fill_color);
                let mut img = RgbaImage::new(w, h);
                for pixel in img.pixels_mut() {
                    *pixel = color;
                }
                img
            }
        }
        FillType::None => {
            RgbaImage::new(w, h)
        }
    };
    Ok(img)
}

/// Create the source image buffer for a layer based on its fill type.
fn create_layer_source(
    layer: &ResolvedLayer,
    image_cache: &mut ImageCache,
    assets_dir: &Path,
    canvas: &RgbaImage,
    fwd: &[[f32; 3]; 3],
    disabled_effects: &[String],
) -> Result<RgbaImage> {
    let scale_x = layer.scale[0].abs();
    let scale_y = layer.scale[1].abs();

    let lift_disabled = disabled_effects.iter().any(|d| d == "Lift");
    let has_lift = !lift_disabled && layer.effects.iter().any(|e| matches!(e.effect_type, crate::model::EffectType::Lift(_)));
    let has_effects = layer.effects.iter().any(|e| !disabled_effects.iter().any(|d| d == e.effect_type.type_name()));

    let (w, h) = if (has_lift || has_effects) && layer.fill_type != FillType::Media {
        let w_scaled = (layer.size[0] * scale_x).max(1.0).round() as u32;
        let h_scaled = (layer.size[1] * scale_y).max(1.0).round() as u32;
        (w_scaled, h_scaled)
    } else {
        (layer.size[0].max(1.0) as u32, layer.size[1].max(1.0) as u32)
    };

    let mut img = if has_lift {
        let lift_params = layer.effects.iter()
            .find_map(|e| {
                if let crate::model::EffectType::Lift(ref params) = e.effect_type {
                    Some(params)
                } else {
                    None
                }
            })
            .expect("Lift effect already confirmed");

        let shape_img = if lift_params.fill > 0.0 {
            Some(create_base_shape_source(layer, w, h, image_cache, assets_dir)?)
        } else {
            None
        };

        apply_lift(w, h, layer.size[0], layer.size[1], lift_params.fill, shape_img, canvas, fwd)?
    } else {
        create_base_shape_source(layer, w, h, image_cache, assets_dir)?
    };

    img = crate::render::effects::apply_pixel_effects(&layer.effects, img, layer, disabled_effects)?;

    Ok(img)
}

/// Render a gradient into an image buffer.
fn render_gradient(w: u32, h: u32, gradient: &Gradient) -> RgbaImage {
    let mut img = RgbaImage::new(w, h);

    if gradient.stops.is_empty() {
        return img;
    }

    let start = gradient.start;
    let end = gradient.end;
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let len_sq = dx * dx + dy * dy;

    for y in 0..h {
        for x in 0..w {
            // Normalize coordinates to [0, 1]
            let nx = x as f32 / w.max(1) as f32;
            let ny = y as f32 / h.max(1) as f32;

            // Compute linear gradient position by projecting onto the segment start -> end
            let t = if len_sq > 1e-6 {
                let px = nx - start[0];
                let py = ny - start[1];
                ((px * dx + py * dy) / len_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let color = sample_gradient(&gradient.stops, t);
            img.put_pixel(x, y, to_rgba_u8(color));
        }
    }

    img
}

/// Sample a gradient at position t, interpolating between stops.
fn sample_gradient(stops: &[crate::model::GradientStop], t: f32) -> [f32; 4] {
    if stops.is_empty() {
        return [0.0, 0.0, 0.0, 0.0];
    }
    if stops.len() == 1 {
        return stops[0].color;
    }

    let t = t.clamp(0.0, 1.0);

    // Before first stop
    if t <= stops[0].position {
        return stops[0].color;
    }
    // After last stop
    if t >= stops[stops.len() - 1].position {
        return stops[stops.len() - 1].color;
    }

    // Find surrounding stops
    for i in 0..stops.len() - 1 {
        if t >= stops[i].position && t <= stops[i + 1].position {
            let range = stops[i + 1].position - stops[i].position;
            let local_t = if range > f32::EPSILON {
                (t - stops[i].position) / range
            } else {
                0.0
            };
            return [
                stops[i].color[0] + (stops[i + 1].color[0] - stops[i].color[0]) * local_t,
                stops[i].color[1] + (stops[i + 1].color[1] - stops[i].color[1]) * local_t,
                stops[i].color[2] + (stops[i + 1].color[2] - stops[i].color[2]) * local_t,
                stops[i].color[3] + (stops[i + 1].color[3] - stops[i].color[3]) * local_t,
            ];
        }
    }

    stops[stops.len() - 1].color
}

/// Resolve an `am-internal:///` URI to an actual image file in the assets directory.
///
/// Tries several strategies:
/// 1. Exact filename match
/// 2. Hash prefix match (first 8 chars)
/// 3. Case-insensitive match
fn load_image_from_uri(uri: &str, assets_dir: &Path) -> Result<RgbaImage> {
    // Extract filename from URI (e.g., "am-internal:///ABC123.PNG" → "ABC123.PNG")
    let filename = uri
        .rsplit("///")
        .next()
        .unwrap_or(uri)
        .trim_start_matches('/');

    // Strategy 1: Direct filename match
    let direct_path = assets_dir.join(filename);
    if direct_path.exists() {
        let img = image::open(&direct_path)
            .with_context(|| format!("Failed to open image: {}", direct_path.display()))?;
        return Ok(img.to_rgba8());
    }

    // Strategy 2: Try without the extension or with different case
    if let Ok(entries) = std::fs::read_dir(assets_dir) {
        let filename_lower = filename.to_lowercase();
        let stem = filename_lower
            .rsplit('.')
            .last()
            .unwrap_or(&filename_lower);

        for entry in entries.flatten() {
            let entry_name = entry.file_name().to_string_lossy().to_string();
            let entry_lower = entry_name.to_lowercase();
            let entry_path = entry.path();

            // Case-insensitive exact match (including extension if present in URI)
            if entry_lower == filename_lower {
                let img = image::open(&entry_path)
                    .with_context(|| {
                        format!("Failed to open image: {}", entry_path.display())
                    })?;
                return Ok(img.to_rgba8());
            }

            // Case-insensitive exact stem match (e.g. "1000174558.jpg" matches "1000174558")
            if let Some(entry_stem) = entry_path.file_stem().and_then(|s| s.to_str()) {
                if entry_stem.to_lowercase() == filename_lower {
                    let img = image::open(&entry_path)
                        .with_context(|| {
                            format!("Failed to open image: {}", entry_path.display())
                        })?;
                    return Ok(img.to_rgba8());
                }
            }

            // Hash prefix match (first 8 characters of the stem)
            if stem.len() >= 8 {
                let prefix = &stem[..8];
                if entry_lower.starts_with(prefix) {
                    let img = image::open(&entry_path)
                        .with_context(|| {
                            format!("Failed to open image: {}", entry_path.display())
                        })?;
                    return Ok(img.to_rgba8());
                }
            }
        }
    }

    bail!(
        "Could not find image for URI '{}' in assets directory '{}'",
        uri,
        assets_dir.display()
    )
}

/// Convert RGBA [f32; 4] (0.0 - 1.0) to image::Rgba<u8>.
fn to_rgba_u8(color: [f32; 4]) -> Rgba<u8> {
    Rgba([
        (color[0] * 255.0).round().clamp(0.0, 255.0) as u8,
        (color[1] * 255.0).round().clamp(0.0, 255.0) as u8,
        (color[2] * 255.0).round().clamp(0.0, 255.0) as u8,
        (color[3] * 255.0).round().clamp(0.0, 255.0) as u8,
    ])
}

/// Parse a hex color string in #AARRGGBB format to RGBA [f32; 4].
///
/// Handles both 8-char (#AARRGGBB) and 6-char (#RRGGBB, assumes full alpha) formats.
/// Note: Alight Motion uses ARGB order, not RGBA!
pub fn parse_hex_color(hex: &str) -> [f32; 4] {
    let hex = hex.trim_start_matches('#');

    match hex.len() {
        8 => {
            // #AARRGGBB → RGBA
            let a = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
            let r = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[6..8], 16).unwrap_or(0);
            [
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                a as f32 / 255.0,
            ]
        }
        6 => {
            // #RRGGBB → RGBA with full alpha
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
        }
        _ => {
            warn!("Invalid hex color format: #{}", hex);
            [0.0, 0.0, 0.0, 1.0]
        }
    }
}

// ---------------------------------------------------------------------------
// Bounding box, line, and pixel font drawing utilities (for debug_layout)
// ---------------------------------------------------------------------------

fn draw_line(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    let mut x = x0;
    let mut y = y0;

    loop {
        if x >= 0 && x < img.width() as i32 && y >= 0 && y < img.height() as i32 {
            img.put_pixel(x as u32, y as u32, color);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

fn draw_rect(img: &mut RgbaImage, x: i32, y: i32, w: i32, h: i32, color: Rgba<u8>) {
    draw_line(img, x, y, x + w, y, color);
    draw_line(img, x + w, y, x + w, y + h, color);
    draw_line(img, x + w, y + h, x, y + h, color);
    draw_line(img, x, y + h, x, y, color);
}

fn get_char_pixels(c: char) -> u16 {
    match c.to_ascii_uppercase() {
        '0' => 0b111_101_101_101_111,
        '1' => 0b010_110_010_010_111,
        '2' => 0b111_001_111_100_111,
        '3' => 0b111_001_111_001_111,
        '4' => 0b101_101_111_001_001,
        '5' => 0b111_100_111_001_111,
        '6' => 0b111_100_111_101_111,
        '7' => 0b111_001_010_010_010,
        '8' => 0b111_101_111_101_111,
        '9' => 0b111_101_111_001_111,
        'A' => 0b111_101_111_101_101,
        'B' => 0b110_101_110_101_110,
        'C' => 0b111_100_100_100_111,
        'D' => 0b110_101_101_101_110,
        'E' => 0b111_100_111_100_111,
        'F' => 0b111_100_111_100_100,
        'G' => 0b111_100_101_101_111,
        'H' => 0b101_101_111_101_101,
        'I' => 0b111_010_010_010_111,
        'J' => 0b001_001_001_101_111,
        'K' => 0b101_101_110_101_101,
        'L' => 0b100_100_100_100_111,
        'M' => 0b101_111_101_101_101,
        'N' => 0b101_111_101_101_101,
        'O' => 0b111_101_101_101_111,
        'P' => 0b111_101_111_100_100,
        'Q' => 0b111_101_111_011_001,
        'R' => 0b111_101_111_101_101,
        'S' => 0b111_100_111_001_111,
        'T' => 0b111_010_010_010_010,
        'U' => 0b101_101_101_101_111,
        'V' => 0b101_101_101_010_010,
        'W' => 0b101_101_101_111_101,
        'X' => 0b101_101_010_101_101,
        'Y' => 0b101_101_010_010_010,
        'Z' => 0b111_001_010_100_111,
        ':' => 0b000_010_000_010_000,
        '-' => 0b000_000_111_000_000,
        '_' => 0b000_000_000_000_111,
        '.' => 0b000_000_000_000_010,
        '(' => 0b010_100_100_100_010,
        ')' => 0b010_001_001_001_010,
        _   => 0b000_000_000_000_000,
    }
}

pub(crate) fn draw_text_with_bg(img: &mut RgbaImage, x: i32, y: i32, text: &str, fg: Rgba<u8>, bg: Rgba<u8>, scale: i32) {
    let w = text.len() as i32 * 4 * scale;
    let h = 5 * scale;
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px >= 0 && px < img.width() as i32 && py >= 0 && py < img.height() as i32 {
                img.put_pixel(px as u32, py as u32, bg);
            }
        }
    }
    draw_text(img, x, y, text, fg, scale);
}

pub(crate) fn draw_char(img: &mut RgbaImage, x: i32, y: i32, c: char, color: Rgba<u8>, scale: i32) {
    let bits = get_char_pixels(c);
    for row in 0..5 {
        for col in 0..3 {
            let bit_idx = 14 - (row * 3 + col);
            let active = ((bits >> bit_idx) & 1) == 1;
            if active {
                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = x + col * scale + dx;
                        let py = y + row * scale + dy;
                        if px >= 0 && px < img.width() as i32 && py >= 0 && py < img.height() as i32 {
                            img.put_pixel(px as u32, py as u32, color);
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn draw_text(img: &mut RgbaImage, x: i32, y: i32, text: &str, color: Rgba<u8>, scale: i32) {
    let mut cur_x = x;
    for c in text.chars() {
        draw_char(img, cur_x, y, c, color, scale);
        cur_x += 4 * scale; // 3 columns + 1 space column
    }
}

/// Draw outline bounding box and label for a layer (used in debug_layout mode)
fn draw_layer_debug_outline(canvas: &mut RgbaImage, layer: &ResolvedLayer, scene_w: u32, scene_h: u32, disabled_effects: &[String]) {
    let canvas_w = canvas.width() as f32;
    let canvas_h = canvas.height() as f32;
    let canvas_center = [canvas_w / 2.0, canvas_h / 2.0];

    let layer_w = layer.size[0];
    let layer_h = layer.size[1];

    if layer_w <= 0.0 || layer_h <= 0.0 {
        return;
    }

    // Apply transform-modifying effects
    let (mut location, mut scale, rotation) = apply_transform_effects(
        &layer.effects,
        layer.location,
        layer.scale,
        layer.rotation,
        layer.time_secs,
        layer.normalized_t,
        disabled_effects,
    );

    // Capture real values before halving for the label
    let rendered_w = layer_w * scale[0].abs();
    let rendered_h = layer_h * scale[1].abs();
    let cx_offset = location[0] - (scene_w as f32 / 2.0);
    let cy_offset = location[1] - (scene_h as f32 / 2.0);

    // Scale locations and sizes by 0.5 and center them within expanded canvas
    let proj_center = [scene_w as f32 / 2.0, scene_h as f32 / 2.0];
    location[0] = (location[0] - proj_center[0]) * 0.5 + canvas_center[0];
    location[1] = (location[1] - proj_center[1]) * 0.5 + canvas_center[1];
    scale[0] *= 0.5;
    scale[1] *= 0.5;

    // Build forward transform: layer-local -> canvas coordinates
    let fwd = build_transform_matrix(location, scale, rotation, canvas_center);

    let half_w = layer_w / 2.0;
    let half_h = layer_h / 2.0;
    let tl = transform_point(&fwd, [-half_w, -half_h]);
    let tr = transform_point(&fwd, [half_w, -half_h]);
    let br = transform_point(&fwd, [half_w, half_h]);
    let bl = transform_point(&fwd, [-half_w, half_h]);

    let colors = [
        Rgba([0, 255, 0, 255]),
        Rgba([0, 255, 255, 255]),
        Rgba([255, 0, 255, 255]),
        Rgba([255, 255, 0, 255]),
        Rgba([255, 128, 0, 255]),
        Rgba([255, 0, 0, 255]),
        Rgba([128, 0, 255, 255]),
    ];
    let color = colors[(layer.id as usize) % colors.len()];

    // Draw outlines
    draw_line(canvas, tl[0] as i32, tl[1] as i32, tr[0] as i32, tr[1] as i32, color);
    draw_line(canvas, tr[0] as i32, tr[1] as i32, br[0] as i32, br[1] as i32, color);
    draw_line(canvas, br[0] as i32, br[1] as i32, bl[0] as i32, bl[1] as i32, color);
    draw_line(canvas, bl[0] as i32, bl[1] as i32, tl[0] as i32, tl[1] as i32, color);

    // Draw pivot crosshair at the layer's transform origin
    let pivot = transform_point(&fwd, [0.0, 0.0]);
    draw_line(canvas, pivot[0] as i32 - 6, pivot[1] as i32, pivot[0] as i32 + 6, pivot[1] as i32, Rgba([255, 255, 255, 180]));
    draw_line(canvas, pivot[0] as i32, pivot[1] as i32 - 6, pivot[0] as i32, pivot[1] as i32 + 6, Rgba([255, 255, 255, 180]));

    // Draw clean text label (two lines)
    let clean_label: String = layer.label.as_deref().unwrap_or("Layer")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
        .collect();

    let label_x = tl[0] as i32 + 5;
    let label_y = tl[1] as i32 + 5;
    let bg = Rgba([26, 26, 26, 230]);

    let line1 = format!("{} {}x{} @ {}%  {}x{}",
        clean_label,
        layer_w as u32, layer_h as u32,
        (rendered_w / layer_w * 100.0) as u32,
        rendered_w as u32, rendered_h as u32);
    draw_text_with_bg(canvas, label_x, label_y, &line1, color, bg, 2);

    let line2 = format!("POS {}:{}  ROT {}d",
        cx_offset as i32, cy_offset as i32,
        rotation as i32);
    draw_text_with_bg(canvas, label_x + 2, label_y + 5 * 2 + 2, &line2, color, bg, 2);
}
