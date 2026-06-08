//! PNG sequence export.

use anyhow::{Context, Result};
use image::RgbaImage;
use std::path::Path;
use rayon::prelude::*;

use crate::model::Project;
use crate::eval::timeline::evaluate;
use crate::render::compositor::{render_scene, ImageCache};

/// Export a single frame image as a PNG file.
pub fn export_frame(image: &RgbaImage, output_dir: &Path, frame_number: u32) -> Result<()> {
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output directory: {}", output_dir.display()))?;
    let path = output_dir.join(format!("frame_{:06}.png", frame_number));
    image.save(&path)
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
    cache: &mut ImageCache,
    debug_layout: bool,
) -> Result<()> {
    let start = start_frame.unwrap_or(0);
    let total_frames = project.total_frames();
    let end = end_frame.unwrap_or(total_frames).min(total_frames);

    let fps = project.fps as f32;

    // Pre-load all media assets into the shared cache (serial, avoids duplicate disk reads)
    // Then clone the cache for each thread — images are already loaded so cloning is cheap.
    // (The virtual_mappings HashMap and loaded RgbaImage buffers are Arc-free but Clone.)

    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output directory: {}", output_dir.display()))?;

    // Build the frame list and capture virtual mappings to share across threads
    let virtual_mappings = cache.virtual_mappings_clone();

    // Render frames in parallel
    let results: Vec<Result<()>> = (start..end)
        .into_par_iter()
        .map(|frame| {
            let time_secs = frame as f32 / fps;
            let resolved = evaluate(project, time_secs);

            // Each thread gets its own cache — images are loaded on demand per thread
            let mut thread_cache = ImageCache::new_with_mappings(virtual_mappings.clone());
            let img = render_scene(&resolved, &mut thread_cache, assets_dir, debug_layout)
                .with_context(|| format!("failed to render frame {}", frame))?;

            let path = output_dir.join(format!("frame_{:06}.png", frame));
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
