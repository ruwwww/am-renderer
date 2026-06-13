//! Video export using FFmpeg.

use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::model::AudioTrack;

/// Shells out to ffmpeg to stitch PNG frames and mixed audio tracks into an MP4 video.
pub fn export_mp4(
    frames_dir: &Path,
    output_path: &Path,
    fps: u32,
    width: u32,
    height: u32,
    audio_tracks: &[AudioTrack],
    assets_dir: &Path,
    virtual_mappings: &HashMap<String, PathBuf>,
) -> Result<()> {
    let input_pattern = frames_dir.join("frame_%06d.png");

    // libx264 requires even dimensions for yuv420p.
    let w = if width % 2 != 0 { width + 1 } else { width };
    let h = if height % 2 != 0 { height + 1 } else { height };

    // Resolve physical paths for each audio track
    let mut resolved_tracks = Vec::new();
    for track in audio_tracks {
        if let Some(ref uri) = track.src {
            let path = if let Some(mapped_path) = virtual_mappings.get(uri) {
                Some(mapped_path.clone())
            } else {
                // Try direct match in assets_dir
                let filename = uri
                    .rsplit("///")
                    .next()
                    .unwrap_or(uri)
                    .trim_start_matches('/');
                let direct_path = assets_dir.join(filename);
                if direct_path.exists() {
                    Some(direct_path)
                } else {
                    // Try case-insensitive filename matches
                    let mut found_path = None;
                    if let Ok(entries) = std::fs::read_dir(assets_dir) {
                        let filename_lower = filename.to_lowercase();
                        for entry in entries.flatten() {
                            let entry_path = entry.path();
                            if entry_path.is_file() {
                                if let Some(name) = entry_path.file_name().and_then(|s| s.to_str()) {
                                    if name.to_lowercase() == filename_lower {
                                        found_path = Some(entry_path);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    found_path
                }
            };

            if let Some(p) = path {
                resolved_tracks.push((track, p));
            } else {
                log::warn!(
                    "Could not resolve physical audio file for track URI: {}",
                    uri
                );
            }
        }
    }

    // Build FFmpeg command arguments
    let mut args = vec![
        "-y".to_string(),
        "-framerate".to_string(),
        fps.to_string(),
        "-i".to_string(),
        input_pattern
            .to_str()
            .ok_or_else(|| anyhow!("invalid frames path"))?
            .to_string(),
    ];

    // Add audio inputs with seeking and duration trimming
    // NOTE: input index 0 is video. Audio inputs start at index 1.
    for (track, path) in &resolved_tracks {
        // Source start offset (trim)
        let in_secs = track.in_time / 1000.0;
        args.push("-ss".to_string());
        args.push(in_secs.to_string());

        // Clip duration on the timeline
        let duration_secs = (track.end_time - track.start_time) / 1000.0;
        args.push("-t".to_string());
        args.push(duration_secs.to_string());

        args.push("-i".to_string());
        args.push(
            path.to_str()
                .ok_or_else(|| anyhow!("invalid audio path"))?
                .to_string(),
        );
    }

    let has_audio = !resolved_tracks.is_empty();
    if has_audio {
        let mut filter_parts = Vec::new();

        // 1. Delay each audio stream to match its timeline start_time
        for (i, (track, _)) in resolved_tracks.iter().enumerate() {
            let input_idx = i + 1; // 0 is video
            let start_ms = track.start_time.round() as u32;
            // adelay delays all channels by start_ms
            filter_parts.push(format!(
                "[{}:a]adelay={}:all=true[a{}]",
                input_idx, start_ms, input_idx
            ));
        }

        // 2. Mix delayed audio streams if N > 1, otherwise map the single stream directly
        let out_label = if resolved_tracks.len() > 1 {
            let mut mix_input_labels = String::new();
            for i in 0..resolved_tracks.len() {
                mix_input_labels.push_str(&format!("[a{}]", i + 1));
            }
            filter_parts.push(format!(
                "{}amix=inputs={}[a_mixed]",
                mix_input_labels,
                resolved_tracks.len()
            ));
            "[a_mixed]"
        } else {
            "[a1]"
        };

        let filter_complex_str = filter_parts.join(";");
        args.push("-filter_complex".to_string());
        args.push(filter_complex_str);

        // Map streams
        args.push("-map".to_string());
        args.push("0:v".to_string());
        args.push("-map".to_string());
        args.push(out_label.to_string());
        args.push("-c:a".to_string());
        args.push("aac".to_string());
    }

    // Video stream configuration
    args.push("-c:v".to_string());
    args.push("libx264".to_string());
    args.push("-pix_fmt".to_string());
    args.push("yuv420p".to_string());
    args.push("-vf".to_string());
    args.push(format!("scale={}:{}", w, h));

    if has_audio {
        args.push("-shortest".to_string());
    }

    // Output file
    args.push(
        output_path
            .to_str()
            .ok_or_else(|| anyhow!("invalid output path"))?
            .to_string(),
    );

    let mut cmd = Command::new("ffmpeg");
    cmd.args(&args);

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
