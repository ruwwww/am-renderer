//! Utility tool to automatically map and copy random media files from a source directory
//! to match the expected asset filenames required by an Alight Motion XML preset.
//!
//! Run with:
//!   cargo run --example auto_pair -- -i presets/preset1.xml -s <source_dir> -o assets

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

// Reuse types from our main library
use am_renderer::parser::parse_xml;

#[derive(Parser, Debug)]
#[command(
    name = "auto-pair",
    about = "Auto-pair random source images/audio to match XML preset requirements"
)]
struct Args {
    /// Path to the Alight Motion XML file.
    #[arg(short, long)]
    input: PathBuf,

    /// Directory containing your random source images and audio files.
    #[arg(short, long)]
    source: PathBuf,

    /// Output assets directory where renamed/paired files will be copied.
    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // 1. Parse XML and find all unique media URIs
    let xml_scene = parse_xml(&args.input)?;

    // Scan for all required media URIs
    let mut image_uris = HashSet::new();
    let mut audio_uris = HashSet::new();

    for shape in xml_scene.shapes() {
        if shape.fill_type.as_deref() == Some("media") || shape.fill_image.is_some() {
            if let Some(ref uri) = shape.fill_image {
                image_uris.insert(uri.clone());
            }
        }
    }
    for audio in xml_scene.audio() {
        if let Some(ref uri) = audio.src {
            audio_uris.insert(uri.clone());
        }
    }
    for media in xml_scene.media() {
        let is_audio = media
            .r#type
            .as_deref()
            .map(|t| t.starts_with("audio/"))
            .unwrap_or(false);
        if is_audio {
            audio_uris.insert(media.uri.clone());
        } else {
            image_uris.insert(media.uri.clone());
        }
    }

    println!("XML Preset requires:");
    println!("  - {} unique image asset(s)", image_uris.len());
    println!("  - {} unique audio asset(s)", audio_uris.len());

    // 2. Scan source directory for random images and audio files
    let mut source_images = Vec::new();
    let mut source_audio = Vec::new();

    if !args.source.exists() {
        return Err(anyhow!(
            "Source directory '{}' does not exist",
            args.source.display()
        ));
    }

    for entry in fs::read_dir(&args.source)?.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext_lower = ext.to_lowercase();
                if matches!(
                    ext_lower.as_str(),
                    "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
                ) {
                    source_images.push(path);
                } else if matches!(ext_lower.as_str(), "mp3" | "wav" | "m4a" | "ogg") {
                    source_audio.push(path);
                }
            }
        }
    }

    println!("Found in source directory:");
    println!("  - {} source image(s)", source_images.len());
    println!("  - {} source audio file(s)", source_audio.len());

    if image_uris.len() > 0 && source_images.is_empty() {
        return Err(anyhow!(
            "Preset requires images but no source images were found in '{}'",
            args.source.display()
        ));
    }
    if audio_uris.len() > 0 && source_audio.is_empty() {
        return Err(anyhow!(
            "Preset requires audio but no source audio files were found in '{}'",
            args.source.display()
        ));
    }

    // 3. Create output directory
    fs::create_dir_all(&args.output)?;

    // Helper function to extract the stem/filename identifier from URI
    let get_uri_identifier = |uri: &str| -> String {
        let last_part = uri.split('/').last().unwrap_or(uri);
        if let Some(idx) = last_part.rfind('.') {
            last_part[..idx].to_string()
        } else {
            last_part.to_string()
        }
    };

    // 4. Pair and copy images
    let mut img_idx = 0;
    for uri in &image_uris {
        let src_path = &source_images[img_idx % source_images.len()];
        let ext = src_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");

        let dest_filename = format!("{}.{}", get_uri_identifier(uri), ext);
        let dest_path = args.output.join(&dest_filename);

        fs::copy(src_path, &dest_path).with_context(|| {
            format!(
                "failed to copy '{}' to '{}'",
                src_path.display(),
                dest_path.display()
            )
        })?;

        println!(
            "Paired image: {} -> {}",
            src_path.file_name().unwrap().to_string_lossy(),
            dest_filename
        );
        img_idx += 1;
    }

    // 5. Pair and copy audio
    let mut audio_idx = 0;
    for uri in &audio_uris {
        let src_path = &source_audio[audio_idx % source_audio.len()];
        let ext = src_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mp3");

        let dest_filename = format!("{}.{}", get_uri_identifier(uri), ext);
        let dest_path = args.output.join(&dest_filename);

        fs::copy(src_path, &dest_path).with_context(|| {
            format!(
                "failed to copy '{}' to '{}'",
                src_path.display(),
                dest_path.display()
            )
        })?;

        println!(
            "Paired audio: {} -> {}",
            src_path.file_name().unwrap().to_string_lossy(),
            dest_filename
        );
        audio_idx += 1;
    }

    println!(
        "\nSuccess! Placed all paired assets in '{}'",
        args.output.display()
    );
    Ok(())
}
