use anyhow::{bail, Context, Result};
use image::RgbaImage;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Cache for loaded source images to avoid re-reading from disk.
#[derive(Clone)]
pub struct ImageCache {
    images: HashMap<String, Arc<RgbaImage>>,
    pub virtual_mappings: HashMap<String, PathBuf>,
}

impl ImageCache {
    /// Create a new empty image cache.
    pub fn new() -> Self {
        Self {
            images: HashMap::new(),
            virtual_mappings: HashMap::new(),
        }
    }

    /// Create a new cache pre-populated with virtual mappings.
    pub fn new_with_mappings(mappings: HashMap<String, PathBuf>) -> Self {
        Self {
            images: HashMap::new(),
            virtual_mappings: mappings,
        }
    }

    /// Set virtual mappings for media URIs to physical files.
    pub fn set_virtual_mappings(&mut self, mappings: HashMap<String, PathBuf>) {
        self.virtual_mappings = mappings;
    }

    /// Clone just the virtual mappings (cheap — no image data).
    pub fn virtual_mappings_clone(&self) -> HashMap<String, PathBuf> {
        self.virtual_mappings.clone()
    }

    /// Load an image by URI, returning a reference to the cached image.
    ///
    /// If the image has already been loaded, returns the cached version.
    /// Otherwise, resolves the URI to a file path within `assets_dir` and loads it.
    pub fn load(&mut self, uri: &str, assets_dir: &Path) -> Result<Arc<RgbaImage>> {
        if !self.images.contains_key(uri) {
            let img = if let Some(physical_path) = self.virtual_mappings.get(uri) {
                image::open(physical_path)
                    .with_context(|| {
                        format!(
                            "Failed to open virtually paired image: {}",
                            physical_path.display()
                        )
                    })?
                    .to_rgba8()
            } else {
                load_image_from_uri(uri, assets_dir)?
            };
            self.images.insert(uri.to_string(), Arc::new(img));
        }
        Ok(Arc::clone(self.images.get(uri).unwrap()))
    }
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
        let stem = filename_lower.rsplit('.').last().unwrap_or(&filename_lower);

        for entry in entries.flatten() {
            let entry_name = entry.file_name().to_string_lossy().to_string();
            let entry_lower = entry_name.to_lowercase();
            let entry_path = entry.path();

            // Case-insensitive exact match (including extension if present in URI)
            if entry_lower == filename_lower {
                let img = image::open(&entry_path)
                    .with_context(|| format!("Failed to open image: {}", entry_path.display()))?;
                return Ok(img.to_rgba8());
            }

            // Case-insensitive exact stem match (e.g. "1000174558.jpg" matches "1000174558")
            if let Some(entry_stem) = entry_path.file_stem().and_then(|s| s.to_str()) {
                if entry_stem.to_lowercase() == filename_lower {
                    let img = image::open(&entry_path).with_context(|| {
                        format!("Failed to open image: {}", entry_path.display())
                    })?;
                    return Ok(img.to_rgba8());
                }
            }

            // Hash prefix match (first 8 characters of the stem)
            if stem.len() >= 8 {
                let prefix = &stem[..8];
                if entry_lower.starts_with(prefix) {
                    let img = image::open(&entry_path).with_context(|| {
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
