use std::path::Path;
use anyhow::Result;
use image::{RgbaImage, Rgba};

use crate::eval::timeline::ResolvedScene;
use crate::render::compositor::{render_scene, ImageCache};
use crate::model::Effect;

const DEBUG_SUBDIR: &str = "debug_effects";
const TILE_GAP: u32 = 4;
const LABEL_H: u32 = 16;

/// Render debug views showing each effect independently and chain order variants.
///
/// For each layer that has effects, creates a subdirectory with:
/// - `layer{N}_no_effects.png` — the scene with this layer's effects removed
/// - `layer{N}_ef{M}_{Type}.png` — only effect M applied (others disabled on this layer)
/// - `layer{N}_chain_reversed.png` — effects applied in reverse order (bottom-up)
/// - `layer{N}_chain_cumulative.png` — single tiled image showing progressive
///   top-down and bottom-up application
pub fn render_effects_debug(
    scene: &ResolvedScene,
    image_cache: &mut ImageCache,
    assets_dir: &Path,
    frame_num: u32,
    output_dir: &Path,
) -> Result<()> {
    let debug_dir = output_dir.join(format!("{}_frame{:06}", DEBUG_SUBDIR, frame_num));
    std::fs::create_dir_all(&debug_dir)?;

    for (layer_idx, layer) in scene.layers.iter().enumerate() {
        if layer.effects.is_empty() {
            continue;
        }

        let base_scene = scene.clone();

        // 1. No effects on this layer
        {
            let mut ms = base_scene.clone();
            ms.layers[layer_idx].effects.clear();
            let mut cc = image_cache.clone();
            let img = render_scene(&ms, &mut cc, assets_dir, false)?;
            let path = debug_dir.join(format!("layer{}_no_effects.png", layer_idx));
            img.save(&path)?;
        }

        // 2. Each effect in isolation
        for (ef_idx, effect) in layer.effects.iter().enumerate() {
            let ef_name = effect_type_short_name(&effect.effect_type);
            let mut ms = base_scene.clone();
            ms.layers[layer_idx].effects.clear();
            ms.layers[layer_idx].effects.push(effect.clone());
            let mut cc = image_cache.clone();
            let img = render_scene(&ms, &mut cc, assets_dir, false)?;
            let path = debug_dir.join(format!(
                "layer{}_ef{}_{}.png", layer_idx, ef_idx, ef_name
            ));
            img.save(&path)?;
        }

        // 3. Chain reversed (bottom-up)
        {
            let mut ms = base_scene.clone();
            let rev: Vec<Effect> = layer.effects.iter().rev().cloned().collect();
            ms.layers[layer_idx].effects = rev;
            let mut cc = image_cache.clone();
            let img = render_scene(&ms, &mut cc, assets_dir, false)?;
            let path = debug_dir.join(format!("layer{}_chain_reversed.png", layer_idx));
            img.save(&path)?;
        }

        // 4. Cumulative chain tiled image (top-down and bottom-up)
        {
            let cumulative_img = build_cumulative_tile(
                &base_scene, layer_idx, &layer.effects, image_cache, assets_dir,
            )?;
            let path = debug_dir.join(format!("layer{}_chain_cumulative.png", layer_idx));
            cumulative_img.save(&path)?;
        }
    }

    Ok(())
}

/// Build a tiled image showing cumulative effect application.
///
/// Top row: effects added one-by-one from top (first, first+second, ... all)
/// Bottom row: effects added one-by-one from bottom (last, last+second-last, ... all)
fn build_cumulative_tile(
    base_scene: &ResolvedScene,
    layer_idx: usize,
    effects: &[Effect],
    image_cache: &mut ImageCache,
    assets_dir: &Path,
) -> Result<RgbaImage> {
    let n = effects.len();

    // First pass: render all cumulative steps to determine sizes
    let mut top_steps: Vec<RgbaImage> = Vec::with_capacity(n);
    let mut bot_steps: Vec<RgbaImage> = Vec::with_capacity(n);

    // Top-down cumulative
    for count in 1..=n {
        let mut ms = base_scene.clone();
        ms.layers[layer_idx].effects = effects[..count].to_vec();
        let mut cc = image_cache.clone();
        let img = render_scene(&ms, &mut cc, assets_dir, false)?;
        top_steps.push(img);
    }

    // Bottom-up cumulative
    let rev: Vec<&Effect> = effects.iter().rev().collect();
    for count in 1..=n {
        let mut ms = base_scene.clone();
        ms.layers[layer_idx].effects = rev[..count].iter().map(|e| (*e).clone()).collect();
        let mut cc = image_cache.clone();
        let img = render_scene(&ms, &mut cc, assets_dir, false)?;
        bot_steps.push(img);
    }

    let tile_w = top_steps[0].width();
    let tile_h = top_steps[0].height();

    let labels_top = (0..n).map(|i| format!("Top{}", i + 1)).collect::<Vec<_>>();
    let labels_bot = (0..n).map(|i| format!("Bot{}", i + 1)).collect::<Vec<_>>();

    let total_w = tile_w.saturating_mul(n as u32).saturating_add(TILE_GAP.saturating_mul((n as u32).saturating_sub(1)));
    let total_h = (tile_h + LABEL_H) * 2 + TILE_GAP * 3;

    let mut canvas = RgbaImage::new(total_w, total_h);
    // Fill with dark background
    for pixel in canvas.pixels_mut() {
        *pixel = Rgba([20, 20, 20, 255]);
    }

    // Draw label text helper using the existing pixel font
    fn draw_label(img: &mut RgbaImage, x: u32, y: u32, text: &str, color: Rgba<u8>) {
        let mut cx = x;
        for c in text.chars() {
            crate::render::compositor::draw_char(img, cx as i32, y as i32, c, color, 1);
            cx += 4;
        }
    }

    // Top row: label + image
    let top_label_y = TILE_GAP;
    let top_img_y = top_label_y + LABEL_H;
    for (i, img) in top_steps.iter().enumerate() {
        let x = i as u32 * (tile_w + TILE_GAP);
        draw_label(&mut canvas, x, top_label_y, &labels_top[i], Rgba([200, 200, 200, 255]));
        for dy in 0..tile_h {
            for dx in 0..tile_w {
                let src_px = img.get_pixel(dx, dy);
                canvas.put_pixel(x + dx, top_img_y + dy, *src_px);
            }
        }
    }

    // Bottom row: label + image
    let bot_label_y = top_img_y + tile_h + TILE_GAP * 2;
    let bot_img_y = bot_label_y + LABEL_H;
    for (i, img) in bot_steps.iter().enumerate() {
        let x = i as u32 * (tile_w + TILE_GAP);
        draw_label(&mut canvas, x, bot_label_y, &labels_bot[i], Rgba([200, 200, 200, 255]));
        for dy in 0..tile_h {
            for dx in 0..tile_w {
                let src_px = img.get_pixel(dx, dy);
                canvas.put_pixel(x + dx, bot_img_y + dy, *src_px);
            }
        }
    }

    Ok(canvas)
}

/// Extract a short, safe name from an EffectType for use in filenames.
fn effect_type_short_name(et: &crate::model::EffectType) -> String {
    let s = format!("{:?}", et);
    let base = s.split('(').next().unwrap_or("unknown");
    base.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}