//! Video export using FFmpeg.

use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::process::Command;

/// Shells out to ffmpeg to stitch PNG frames into an MP4 video.
pub fn export_mp4(
    frames_dir: &Path,
    output_path: &Path,
    fps: u32,
    width: u32,
    height: u32,
) -> Result<()> {
    let input_pattern = frames_dir.join("frame_%06d.png");

    // libx264 requires even dimensions for yuv420p.
    let w = if width % 2 != 0 { width + 1 } else { width };
    let h = if height % 2 != 0 { height + 1 } else { height };

    let mut cmd = Command::new("ffmpeg");
    cmd.args(&[
        "-y",
        "-framerate",
        &fps.to_string(),
        "-i",
        input_pattern.to_str().ok_or_else(|| anyhow!("invalid frames path"))?,
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-vf",
        &format!("scale={}:{}", w, h),
        output_path.to_str().ok_or_else(|| anyhow!("invalid output path"))?,
    ]);

    log::info!("Running ffmpeg command: {:?}", cmd);

    let output = cmd.output().with_context(|| {
        "failed to execute ffmpeg process. Make sure ffmpeg is installed and in your PATH."
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("ffmpeg failed: {}", stderr));
    }

    Ok(())
}
