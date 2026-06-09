use std::path::{Path, PathBuf};
use anyhow::Result;
use image::{RgbaImage, Rgba};
use rayon::prelude::*;

use crate::eval::timeline::ResolvedScene;
use crate::render::compositor::{render_scene, ImageCache};
use crate::model::Effect;

const DEBUG_SUBDIR: &str = "debug_effects";
const TILE_GAP: u32 = 4;
const LABEL_H: u32 = 16;

struct RenderTask {
    name: String,
    scene: ResolvedScene,
    cache: ImageCache,
    assets_dir: PathBuf,
}

/// Render debug views showing each effect independently and chain order variants.
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
        let mut tasks: Vec<RenderTask> = Vec::new();

        // 1. No effects on this layer
        {
            let mut ms = base_scene.clone();
            ms.layers[layer_idx].effects.clear();
            tasks.push(RenderTask {
                name: "no_effects".to_string(),
                scene: ms,
                cache: image_cache.clone(),
                assets_dir: assets_dir.to_path_buf(),
            });
        }

        // 2. Each effect in isolation
        for (ef_idx, effect) in layer.effects.iter().enumerate() {
            let ef_name = effect_type_short_name(&effect.effect_type);
            let filename = format!("ef{}_{}", ef_idx, ef_name);
            let mut ms = base_scene.clone();
            ms.layers[layer_idx].effects.clear();
            ms.layers[layer_idx].effects.push(effect.clone());
            tasks.push(RenderTask {
                name: filename,
                scene: ms,
                cache: image_cache.clone(),
                assets_dir: assets_dir.to_path_buf(),
            });
        }

        // 3. Chain reversed (bottom-up)
        {
            let mut ms = base_scene.clone();
            let rev: Vec<Effect> = layer.effects.iter().rev().cloned().collect();
            ms.layers[layer_idx].effects = rev;
            tasks.push(RenderTask {
                name: "chain_reversed".to_string(),
                scene: ms,
                cache: image_cache.clone(),
                assets_dir: assets_dir.to_path_buf(),
            });
        }

        // Render all independent tasks in parallel
        let results: Vec<(String, RgbaImage)> = tasks
            .par_iter()
            .map(|task| {
                let mut cc = task.cache.clone();
                let img = render_scene(&task.scene, &mut cc, &task.assets_dir, false)
                    .unwrap_or_else(|_| RgbaImage::new(1, 1));
                (task.name.clone(), img)
            })
            .collect();

        for (name, img) in &results {
            let path = debug_dir.join(format!("layer{}_{}.png", layer_idx, name));
            let _ = img.save(&path);
        }

        // 4. Cumulative chain tiled image
        if !layer.effects.is_empty() {
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
fn build_cumulative_tile(
    base_scene: &ResolvedScene,
    layer_idx: usize,
    effects: &[Effect],
    image_cache: &mut ImageCache,
    assets_dir: &Path,
) -> Result<RgbaImage> {
    let n = effects.len();

    // Collect all cumulative step tasks
    let steps: Vec<(usize, bool, ResolvedScene)> = (0..n)
        .flat_map(|count| {
            let idx = count + 1;
            let mut top_scene = base_scene.clone();
            top_scene.layers[layer_idx].effects = effects[..idx].to_vec();

            let rev: Vec<&Effect> = effects.iter().rev().collect();
            let mut bot_scene = base_scene.clone();
            bot_scene.layers[layer_idx].effects = rev[..idx].iter().map(|e| (*e).clone()).collect();

            vec![(idx, true, top_scene), (idx, false, bot_scene)]
        })
        .collect();

    let mut step_results: Vec<(usize, bool, RgbaImage)> = steps
        .par_iter()
        .map(|(count, is_top, scene)| {
            let mut cc = image_cache.clone();
            let img = render_scene(scene, &mut cc, assets_dir, false)
                .unwrap_or_else(|_| RgbaImage::new(1, 1));
            (*count, *is_top, img)
        })
        .collect();

    step_results.sort_by_key(|(count, is_top, _)| (*count, !is_top));

    let mut top_steps: Vec<RgbaImage> = Vec::with_capacity(n);
    let mut bot_steps: Vec<RgbaImage> = Vec::with_capacity(n);
    for (_, is_top, img) in step_results {
        if is_top {
            top_steps.push(img);
        } else {
            bot_steps.push(img);
        }
    }

    let tile_w = top_steps[0].width();
    let tile_h = top_steps[0].height();

    let total_w = tile_w.saturating_mul(n as u32).saturating_add(TILE_GAP.saturating_mul((n as u32).saturating_sub(1)));
    let total_h = (tile_h + LABEL_H) * 2 + TILE_GAP * 3;

    let mut canvas = RgbaImage::new(total_w, total_h);
    for pixel in canvas.pixels_mut() {
        *pixel = Rgba([20, 20, 20, 255]);
    }

    fn draw_label(img: &mut RgbaImage, x: u32, y: u32, text: &str, color: Rgba<u8>) {
        let mut cx = x;
        for c in text.chars() {
            crate::render::compositor::draw_char(img, cx as i32, y as i32, c, color, 1);
            cx += 4;
        }
    }

    let top_label_y = TILE_GAP;
    let top_img_y = top_label_y + LABEL_H;
    for (i, img) in top_steps.iter().enumerate() {
        let x = i as u32 * (tile_w + TILE_GAP);
        draw_label(&mut canvas, x, top_label_y, &format!("Top{}", i + 1), Rgba([200, 200, 200, 255]));
        for dy in 0..tile_h {
            for dx in 0..tile_w {
                canvas.put_pixel(x + dx, top_img_y + dy, *img.get_pixel(dx, dy));
            }
        }
    }

    let bot_label_y = top_img_y + tile_h + TILE_GAP * 2;
    let bot_img_y = bot_label_y + LABEL_H;
    for (i, img) in bot_steps.iter().enumerate() {
        let x = i as u32 * (tile_w + TILE_GAP);
        draw_label(&mut canvas, x, bot_label_y, &format!("Bot{}", i + 1), Rgba([200, 200, 200, 255]));
        for dy in 0..tile_h {
            for dx in 0..tile_w {
                canvas.put_pixel(x + dx, bot_img_y + dy, *img.get_pixel(dx, dy));
            }
        }
    }

    Ok(canvas)
}

fn effect_type_short_name(et: &crate::model::EffectType) -> String {
    let s = format!("{:?}", et);
    let base = s.split('(').next().unwrap_or("unknown");
    base.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}