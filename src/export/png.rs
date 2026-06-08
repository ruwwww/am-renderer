//! PNG sequence export.

use anyhow::{Context, Result};
use image::RgbaImage;
use std::path::Path;

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

    for frame in start..end {
        let time_secs = frame as f32 / fps;
        let resolved = evaluate(project, time_secs);
        let img = render_scene(&resolved, cache, assets_dir, debug_layout)
            .with_context(|| format!("failed to render frame {}", frame))?;
        export_frame(&img, output_dir, frame)?;
    }

    Ok(())
}
