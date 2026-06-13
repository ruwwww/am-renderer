use am_renderer::parser::parse_xml;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let presets_dir = Path::new("presets");
    let entries = fs::read_dir(presets_dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("xml") {
            println!("========================================");
            println!("Analyzing Preset: {}", path.display());
            println!("========================================");

            let scene = parse_xml(&path)?;
            println!("Scene Title: {:?}", scene.title);
            println!("Scene Size: {}x{}", scene.width, scene.height);
            println!("Total shapes: {}", scene.shapes().len());

            let mut media_shapes = 0;
            let mut shapes_with_size = 0;
            let mut shapes_without_size = 0;

            for shape in scene.shapes() {
                let is_media =
                    shape.fill_type.as_deref() == Some("media") || shape.fill_image.is_some();
                if is_media {
                    media_shapes += 1;
                }

                let has_size = shape.properties.iter().any(|p| p.name == "size");
                if has_size {
                    shapes_with_size += 1;
                } else {
                    shapes_without_size += 1;
                    println!(
                        "  Shape ID {} (label: {:?}, type: {:?}): NO 'size' property!",
                        shape.id, shape.label, shape.s
                    );
                    // Print other properties to see what defines its dimensions
                    for prop in &shape.properties {
                        println!("    Property: name='{}', value={:?}", prop.name, prop.value);
                    }
                }
            }

            println!(
                "Summary for {}:",
                path.file_name().unwrap().to_string_lossy()
            );
            println!("  Media shapes: {}", media_shapes);
            println!("  Shapes with size: {}", shapes_with_size);
            println!("  Shapes without size: {}", shapes_without_size);
            println!();
        }
    }

    Ok(())
}
