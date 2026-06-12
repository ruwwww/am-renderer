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

pub mod cache;
pub mod gradient;
pub mod shape;
pub mod utils;

pub use cache::ImageCache;
pub use utils::parse_hex_color;

use crate::eval::timeline::ResolvedScene;
use crate::eval::transform::{build_transform_matrix, invert_transform, transform_point};
use crate::model::{FillType, ResolvedLayer};
use crate::render::blending::blend_pixel;
use crate::render::debug_layout::{draw_layer_debug_outline, draw_rect, draw_text};
use crate::render::effects::lift::apply_lift;
use crate::render::effects::transform::apply_transform_effects;
use anyhow::Result;
use image::{Rgba, RgbaImage};
use log::{debug, warn};
use rayon::prelude::*;
use std::path::Path;

use shape::create_base_shape_source;
use utils::to_rgba_u8;

fn calculate_viewport_bounds(scene: &ResolvedScene, disabled_effects: &[String]) -> (f32, f32, f32, f32) {
    let scene_w = scene.width as f32;
    let scene_h = scene.height as f32;

    let mut viewport_xmin = -0.1 * scene_w;
    let mut viewport_xmax = 1.1 * scene_w;
    let mut viewport_ymin = -0.1 * scene_h;
    let mut viewport_ymax = 1.1 * scene_h;

    for layer in &scene.layers {
        let layer_w = layer.size[0];
        let layer_h = layer.size[1];
        if layer_w <= 0.0 || layer_h <= 0.0 {
            continue;
        }

        let (location, scale, rotation) = apply_transform_effects(
            &layer.effects,
            layer.location,
            layer.scale,
            layer.rotation,
            layer.time_secs,
            layer.normalized_t,
            disabled_effects,
        );

        let proj_center = [scene_w / 2.0, scene_h / 2.0];
        let fwd = build_transform_matrix(location, scale, rotation, proj_center);

        let half_w = layer_w / 2.0;
        let half_h = layer_h / 2.0;

        let corners = [
            transform_point(&fwd, [-half_w, -half_h]),
            transform_point(&fwd, [half_w, -half_h]),
            transform_point(&fwd, [half_w, half_h]),
            transform_point(&fwd, [-half_w, half_h]),
        ];

        let mut l_xmin = corners[0][0];
        let mut l_xmax = corners[0][0];
        let mut l_ymin = corners[0][1];
        let mut l_ymax = corners[0][1];

        for c in &corners[1..] {
            l_xmin = l_xmin.min(c[0]);
            l_xmax = l_xmax.max(c[0]);
            l_ymin = l_ymin.min(c[1]);
            l_ymax = l_ymax.max(c[1]);
        }

        if l_xmin < 0.0 {
            viewport_xmin = viewport_xmin.min(l_xmin - 0.2 * scene_w);
        }
        if l_xmax > scene_w {
            viewport_xmax = viewport_xmax.max(l_xmax + 0.2 * scene_w);
        }
        if l_ymin < 0.0 {
            viewport_ymin = viewport_ymin.min(l_ymin - 0.2 * scene_h);
        }
        if l_ymax > scene_h {
            viewport_ymax = viewport_ymax.max(l_ymax + 0.2 * scene_h);
        }
    }

    (viewport_xmin, viewport_xmax, viewport_ymin, viewport_ymax)
}

/// Renders a resolved scene to an RGBA image.
pub fn render_scene(
    scene: &ResolvedScene,
    image_cache: &mut ImageCache,
    assets_dir: &Path,
    debug_layout: bool,
    disabled_effects: &[String],
) -> Result<RgbaImage> {
    let (viewport_xmin, viewport_xmax, viewport_ymin, viewport_ymax) = if debug_layout {
        calculate_viewport_bounds(scene, disabled_effects)
    } else {
        (0.0, scene.width as f32, 0.0, scene.height as f32)
    };

    let canvas_w = (viewport_xmax - viewport_xmin).round() as u32;
    let canvas_h = (viewport_ymax - viewport_ymin).round() as u32;
    let mut canvas = RgbaImage::new(canvas_w, canvas_h);

    if debug_layout {
        // Fill outer background with dark gray
        for pixel in canvas.pixels_mut() {
            *pixel = Rgba([26, 26, 26, 255]);
        }

        // Draw canvas interior filled with project bg color (at 1.0x scale in center of expanded canvas)
        let x0 = (-viewport_xmin).round() as u32;
        let x1 = (scene.width as f32 - viewport_xmin).round() as u32;
        let y0 = (-viewport_ymin).round() as u32;
        let y1 = (scene.height as f32 - viewport_ymin).round() as u32;

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
        if let Err(e) = render_layer(
            &mut comp_canvas,
            layer,
            image_cache,
            assets_dir,
            scene.width,
            scene.height,
            viewport_xmin,
            viewport_ymin,
            debug_layout,
            disabled_effects,
        ) {
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
        let x0 = (-viewport_xmin).round() as u32;
        let x1 = (scene.width as f32 - viewport_xmin).round() as u32;
        let y0 = (-viewport_ymin).round() as u32;
        let y1 = (scene.height as f32 - viewport_ymin).round() as u32;

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
            draw_layer_debug_outline(&mut canvas, layer, scene.width, scene.height, viewport_xmin, viewport_ymin, disabled_effects);
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
    _scene_w: u32,
    _scene_h: u32,
    viewport_xmin: f32,
    viewport_ymin: f32,
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
    let (mut location, scale, rotation) = apply_transform_effects(
        &layer.effects,
        layer.location,
        layer.scale,
        layer.rotation,
        layer.time_secs,
        layer.normalized_t,
        disabled_effects,
    );

    if debug_layout {
        location[0] = location[0] - viewport_xmin;
        location[1] = location[1] - viewport_ymin;
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
