//! Main software compositor — renders resolved scenes to RGBA images.
//!
//! Uses inverse-transform sampling: for each canvas pixel, compute where it
//! maps in the source layer, sample the source, then blend onto the canvas.

use crate::eval::timeline::ResolvedScene;
use crate::eval::transform::{build_transform_matrix, invert_transform, transform_point};
use crate::eval::effects::apply_transform_effects;
use crate::model::{ResolvedLayer, FillType, Gradient};
use crate::render::blending::blend_pixel;
use image::{RgbaImage, Rgba};
use anyhow::{Context, Result, bail};
use std::path::Path;
use std::collections::HashMap;
use log::{debug, warn};

/// Cache for loaded source images to avoid re-reading from disk.
pub struct ImageCache {
    images: HashMap<String, RgbaImage>,
    virtual_mappings: HashMap<String, std::path::PathBuf>,
}

impl ImageCache {
    /// Create a new empty image cache.
    pub fn new() -> Self {
        Self {
            images: HashMap::new(),
            virtual_mappings: HashMap::new(),
        }
    }

    /// Set virtual mappings for media URIs to physical files.
    pub fn set_virtual_mappings(&mut self, mappings: HashMap<String, std::path::PathBuf>) {
        self.virtual_mappings = mappings;
    }

    /// Load an image by URI, returning a reference to the cached image.
    ///
    /// If the image has already been loaded, returns the cached version.
    /// Otherwise, resolves the URI to a file path within `assets_dir` and loads it.
    pub fn load(&mut self, uri: &str, assets_dir: &Path) -> Result<&RgbaImage> {
        if !self.images.contains_key(uri) {
            let img = if let Some(physical_path) = self.virtual_mappings.get(uri) {
                image::open(physical_path)
                    .with_context(|| format!("Failed to open virtually paired image: {}", physical_path.display()))?
                    .to_rgba8()
            } else {
                load_image_from_uri(uri, assets_dir)?
            };
            self.images.insert(uri.to_string(), img);
        }
        Ok(self.images.get(uri).unwrap())
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
) -> Result<RgbaImage> {
    let mut canvas = RgbaImage::new(scene.width, scene.height);

    // Fill with background color
    let bg = to_rgba_u8(scene.bg_color);
    for pixel in canvas.pixels_mut() {
        *pixel = bg;
    }

    // Composite layers bottom to top
    for layer in &scene.layers {
        if let Err(e) = render_layer(&mut canvas, layer, image_cache, assets_dir) {
            warn!("Failed to render layer '{}' (id={}): {}", layer.label.as_deref().unwrap_or("unnamed"), layer.id, e);
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
    let (location, scale, rotation) = apply_transform_effects(
        &layer.effects,
        layer.location,
        layer.scale,
        layer.rotation,
        0.0, // time_secs — we'd need to pass this through for full accuracy
        0.0, // normalized_t — same caveat
    );

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
    let source = create_layer_source(layer, image_cache, assets_dir)?;
    let src_w = source.width() as f32;
    let src_h = source.height() as f32;

    // Half-size for centering (layer's origin is its center)
    let half_w = layer_w / 2.0;
    let half_h = layer_h / 2.0;

    // For each canvas pixel, inverse-transform to find the source coordinate
    for cy in 0..canvas.height() {
        for cx in 0..canvas.width() {
            let canvas_pt = [cx as f32 + 0.5, cy as f32 + 0.5];

            // Map canvas pixel to layer-local coordinates
            let local = transform_point(&inv, canvas_pt);

            // Layer-local coords: origin at layer center, so offset to [0, layer_size]
            let lx = local[0] + half_w;
            let ly = local[1] + half_h;

            // Check if we're inside the layer bounds
            if lx < 0.0 || lx >= layer_w || ly < 0.0 || ly >= layer_h {
                continue;
            }

            // Map layer-local coords to source image coords based on mediaFillMode
            let (sx, sy) = match layer.media_fill_mode.as_deref() {
                Some("fit") => {
                    let scale = (layer_w / src_w).min(layer_h / src_h);
                    let offset_x = (layer_w - src_w * scale) / 2.0;
                    let offset_y = (layer_h - src_h * scale) / 2.0;
                    let sx_f = (lx - offset_x) / scale;
                    let sy_f = (ly - offset_y) / scale;
                    if sx_f < 0.0 || sx_f >= src_w || sy_f < 0.0 || sy_f >= src_h {
                        continue; // Letterbox/pillarbox space, leave transparent
                    }
                    (
                        sx_f.min(src_w - 1.0).max(0.0) as u32,
                        sy_f.min(src_h - 1.0).max(0.0) as u32,
                    )
                }
                Some("fill") => {
                    let scale = (layer_w / src_w).max(layer_h / src_h);
                    let offset_x = (src_w * scale - layer_w) / 2.0;
                    let offset_y = (src_h * scale - layer_h) / 2.0;
                    let sx_f = (lx + offset_x) / scale;
                    let sy_f = (ly + offset_y) / scale;
                    (
                        sx_f.min(src_w - 1.0).max(0.0) as u32,
                        sy_f.min(src_h - 1.0).max(0.0) as u32,
                    )
                }
                _ => {
                    // Default to "stretch": stretch non-uniformly
                    let sx = (lx / layer_w * src_w).min(src_w - 1.0).max(0.0) as u32;
                    let sy = (ly / layer_h * src_h).min(src_h - 1.0).max(0.0) as u32;
                    (sx, sy)
                }
            };

            let src_pixel = *source.get_pixel(sx, sy);

            // Skip fully transparent source pixels
            if src_pixel[3] == 0 {
                continue;
            }

            let dst_pixel = *canvas.get_pixel(cx, cy);
            let blended = blend_pixel(dst_pixel, src_pixel, layer.blend_mode, layer.opacity);
            canvas.put_pixel(cx, cy, blended);
        }
    }

    Ok(())
}

/// Create the source image buffer for a layer based on its fill type.
fn create_layer_source(
    layer: &ResolvedLayer,
    image_cache: &mut ImageCache,
    assets_dir: &Path,
) -> Result<RgbaImage> {
    let w = layer.size[0].max(1.0) as u32;
    let h = layer.size[1].max(1.0) as u32;

    match layer.fill_type {
        FillType::Media => {
            if let Some(ref uri) = layer.fill_image {
                let source = image_cache.load(uri, assets_dir)?;
                Ok(source.clone())
            } else {
                // No media URI — create a transparent placeholder
                warn!("Media layer '{}' has no fill image URI", layer.label.as_deref().unwrap_or("unnamed"));
                Ok(RgbaImage::new(w, h))
            }
        }
        FillType::Color => {
            let color = to_rgba_u8(layer.fill_color);
            let mut img = RgbaImage::new(w, h);
            for pixel in img.pixels_mut() {
                *pixel = color;
            }
            Ok(img)
        }
        FillType::Gradient => {
            if let Some(ref gradient) = layer.gradient {
                Ok(render_gradient(w, h, gradient))
            } else {
                // Fallback to fill color
                let color = to_rgba_u8(layer.fill_color);
                let mut img = RgbaImage::new(w, h);
                for pixel in img.pixels_mut() {
                    *pixel = color;
                }
                Ok(img)
            }
        }
        FillType::None => {
            Ok(RgbaImage::new(w, h))
        }
    }
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
