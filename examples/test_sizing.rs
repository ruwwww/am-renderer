use image::GenericImageView;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Checking resolutions of images in 'assets/':");
    let assets_dir = Path::new("assets");
    if assets_dir.exists() {
        for entry in fs::read_dir(assets_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("png") {
                if let Ok(img) = image::open(&path) {
                    let (w, h) = img.dimensions();
                    println!(
                        "  {}: {}x{} (Aspect Ratio: {:.3})",
                        path.file_name().unwrap().to_string_lossy(),
                        w,
                        h,
                        w as f32 / h as f32
                    );
                }
            }
        }
    } else {
        println!("'assets/' directory does not exist!");
    }
    Ok(())
}
