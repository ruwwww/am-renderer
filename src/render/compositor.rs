//! Main software compositor — renders resolved scenes to RGBA images.
//!
//! Uses inverse-transform sampling: for each canvas pixel, compute where it
//! maps in the source layer, sample the source, then blend onto the canvas.

use crate::eval::timeline::ResolvedScene;
use crate::eval::transform::{build_transform_matrix, invert_transform, transform_point};
use crate::eval::effects::apply_transform_effects;
use crate::model::{ResolvedLayer, FillType, Gradient, EffectType};
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
    debug_layout: bool,
) -> Result<RgbaImage> {
    let mut canvas = RgbaImage::new(scene.width, scene.height);

    if debug_layout {
        // Fill outer background with dark gray
        for pixel in canvas.pixels_mut() {
            *pixel = Rgba([26, 26, 26, 255]);
        }

        // Draw canvas interior filled with project bg color (at 0.5x scale in center)
        let canvas_w = scene.width as f32;
        let canvas_h = scene.height as f32;
        let cx = canvas_w / 2.0;
        let cy = canvas_h / 2.0;

        let x0 = (cx - canvas_w * 0.25) as u32;
        let x1 = (cx + canvas_w * 0.25) as u32;
        let y0 = (cy - canvas_h * 0.25) as u32;
        let y1 = (cy + canvas_h * 0.25) as u32;

        let project_bg = to_rgba_u8(scene.bg_color);
        for y in y0..y1 {
            for x in x0..x1 {
                if x < canvas.width() && y < canvas.height() {
                    canvas.put_pixel(x, y, project_bg);
                }
            }
        }

        // Draw light gray outline around the canvas boundaries
        draw_rect(&mut canvas, x0 as i32, y0 as i32, (x1 - x0) as i32, (y1 - y0) as i32, Rgba([200, 200, 200, 255]));
        // Label the canvas boundary
        draw_text(&mut canvas, x0 as i32 + 10, y0 as i32 + 10, "CANVAS FRAME BOUNDARY", Rgba([200, 200, 200, 255]), 2);
    } else {
        // Fill with background color
        let bg = to_rgba_u8(scene.bg_color);
        for pixel in canvas.pixels_mut() {
            *pixel = bg;
        }
    }

    // Composite layers bottom to top
    for layer in &scene.layers {
        if let Err(e) = render_layer(&mut canvas, layer, image_cache, assets_dir, debug_layout) {
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
    debug_layout: bool,
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
        0.0, // time_secs — we'd need to pass this through for full accuracy
        0.0, // normalized_t — same caveat
    );

    if debug_layout {
        location[0] = (location[0] - canvas_center[0]) * 0.5 + canvas_center[0];
        location[1] = (location[1] - canvas_center[1]) * 0.5 + canvas_center[1];
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

    if debug_layout {
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

        // Label
        let clean_label: String = layer.label.as_deref().unwrap_or("Layer")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
            .collect();
        let label_text = format!("{}:{}", clean_label, layer.id);
        draw_text(canvas, tl[0] as i32 + 5, tl[1] as i32 + 5, &label_text, color, 2);
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

    let mut img = match layer.fill_type {
        FillType::Media => {
            if let Some(ref uri) = layer.fill_image {
                let source = image_cache.load(uri, assets_dir)?;
                source.clone()
            } else {
                // No media URI — create a transparent placeholder
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
                // Fallback to fill color
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

    // Apply pixel-space effects
    for effect in &layer.effects {
        match &effect.effect_type {
            EffectType::Exposure(params) => {
                let exp = params.exposure.evaluate(0.0);
                img = crate::render::effects::color::apply_exposure(&img, exp);
            }
            EffectType::GaussianBlur(params) => {
                img = crate::render::effects::blur::gaussian_blur(&img, params.radius);
            }
            EffectType::Vignette(params) => {
                img = crate::render::effects::color::apply_vignette(&img, params.strength, params.scale);
            }
            EffectType::BrightnessContrast(params) => {
                img = crate::render::effects::color::apply_brightness_contrast(&img, params.brightness, params.contrast);
            }
            EffectType::SaturationVibrance(params) => {
                img = crate::render::effects::color::apply_hsl(&img, 0.0, params.saturation, 0.0);
            }
            EffectType::ColorTint(params) => {
                let color = [params.tint[0], params.tint[1], params.tint[2], 1.0];
                img = crate::render::effects::color::apply_color_fill(&img, color, 1.0);
            }
            _ => {}
        }
    }

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

fn draw_char(img: &mut RgbaImage, x: i32, y: i32, c: char, color: Rgba<u8>, scale: i32) {
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

fn draw_text(img: &mut RgbaImage, x: i32, y: i32, text: &str, color: Rgba<u8>, scale: i32) {
    let mut cur_x = x;
    for c in text.chars() {
        draw_char(img, cur_x, y, c, color, scale);
        cur_x += 4 * scale; // 3 columns + 1 space column
    }
}
