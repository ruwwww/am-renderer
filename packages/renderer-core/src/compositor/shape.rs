use anyhow::Result;
use image::{Rgba, RgbaImage};
use log::warn;
use std::path::Path;

use super::cache::ImageCache;
use super::gradient::render_gradient;
use super::utils::to_rgba_u8;
use graph_resolver::model::{FillType, ResolvedLayer};

/// Create the base shape image source without any lift (background copy) effect.
pub(crate) fn create_base_shape_source(
    layer: &ResolvedLayer,
    w: u32,
    h: u32,
    image_cache: &mut ImageCache,
    assets_dir: &Path,
) -> Result<RgbaImage> {
    let mut img = match layer.fill_type {
        FillType::Media => {
            if let Some(ref uri) = layer.fill_image {
                let source = image_cache.load(uri, assets_dir)?;
                (*source).clone()
            } else {
                warn!(
                    "Media layer '{}' has no fill image URI",
                    layer.label.as_deref().unwrap_or("unnamed")
                );
                RgbaImage::new(w, h)
            }
        }
        FillType::Color => {
            let color = to_rgba_u8(layer.fill_color);
            let mut img = RgbaImage::new(w, h);
            for pixel in img.pixels_mut() {
                *pixel = color;
            }
            img
        }
        FillType::Gradient => {
            if let Some(ref gradient) = layer.gradient {
                render_gradient(w, h, gradient)
            } else {
                let color = to_rgba_u8(layer.fill_color);
                let mut img = RgbaImage::new(w, h);
                for pixel in img.pixels_mut() {
                    *pixel = color;
                }
                img
            }
        }
        FillType::None => RgbaImage::new(w, h),
    };

    if layer.s.as_deref() == Some(".circle") {
        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        let rx = cx.max(1.0);
        let ry = cy.max(1.0);

        for y in 0..h {
            let dy = y as f32 - cy;
            let ny = dy / ry;
            for x in 0..w {
                let dx = x as f32 - cx;
                let nx = dx / rx;
                if nx * nx + ny * ny > 1.0 {
                    img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                }
            }
        }
    }

    Ok(img)
}
