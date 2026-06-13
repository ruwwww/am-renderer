//! PNG sequence export.

use anyhow::{Context, Result};
use image::RgbaImage;
use rayon::prelude::*;
use std::path::Path;

use graph_resolver::eval::timeline::evaluate;
use graph_resolver::model::Project;
use renderer_core::compositor::{render_scene, ImageCache};

/// Export a single frame image as a PNG file.
pub fn export_frame(image: &RgbaImage, output_dir: &Path, frame_number: u32) -> Result<()> {
    std::fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "failed to create output directory: {}",
            output_dir.display()
        )
    })?;
    let path = output_dir.join(format!("frame_{:06}.png", frame_number));
    image
        .save(&path)
        .with_context(|| format!("failed to save frame to: {}", path.display()))?;
    Ok(())
}

/// Export a sequence of frames for a project as PNG files.
///
/// Frames are rendered in parallel (one thread per frame) using rayon,
/// then each rendered image is saved to disk. The ImageCache is cloned
/// per thread since it holds pre-loaded images — this avoids mutex contention
/// at the cost of each thread doing its own image load on first access (which
/// is then cached for that thread's frames).
pub fn export_sequence(
    project: &Project,
    assets_dir: &Path,
    output_dir: &Path,
    start_frame: Option<u32>,
    end_frame: Option<u32>,
    reindex: bool,
    cache: &mut ImageCache,
    debug_layout: bool,
    disabled_effects: &[String],
) -> Result<()> {
    let start = start_frame.unwrap_or(0);
    let total_frames = project.total_frames();
    let end = end_frame.unwrap_or(total_frames).min(total_frames);

    let fps = project.fps as f32;

    // Pre-load all media assets into the shared cache (serial, avoids duplicate disk reads)
    for layer in &project.layers {
        if layer.fill_type == graph_resolver::model::FillType::Media {
            if let Some(ref uri) = layer.fill_image {
                let _ = cache.load(uri, assets_dir);
            }
        }
    }

    std::fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    let de = disabled_effects.to_vec();
    // Render frames in parallel
    let results: Vec<Result<()>> = (start..end)
        .into_par_iter()
        .map(|frame| {
            let time_secs = frame as f32 / fps;
            let resolved = evaluate(project, time_secs);

            // Clone the pre-populated cache (extremely cheap, just increments Arc refs)
            let mut thread_cache = cache.clone();
            let img = render_scene(&resolved, &mut thread_cache, assets_dir, debug_layout, &de)
                .with_context(|| format!("failed to render frame {}", frame))?;

            let file_index = if reindex { frame - start } else { frame };
            let path = output_dir.join(format!("frame_{:06}.png", file_index));
            img.save(&path)
                .with_context(|| format!("failed to save frame to: {}", path.display()))?;
            Ok(())
        })
        .collect();

    // Propagate first error if any
    for r in results {
        r?;
    }

    Ok(())
}
