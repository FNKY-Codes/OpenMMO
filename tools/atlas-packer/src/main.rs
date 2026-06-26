use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct AtlasManifest {
    image: String,
    width: u32,
    height: u32,
    sprites: Vec<SpriteEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SpriteEntry {
    name: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn main() -> anyhow::Result<()> {
    let input = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "assets/sprites".to_string()),
    );
    let output = PathBuf::from(
        std::env::args()
            .nth(2)
            .unwrap_or_else(|| "assets/atlas".to_string()),
    );
    fs::create_dir_all(&output)?;

    let mut sprites = Vec::new();
    let mut y = 0u32;
    let tile = 32u32;
    if input.exists() {
        for entry in fs::read_dir(&input)? {
            let entry = entry?;
            let path = entry.path();
            if is_image(&path) {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("sprite")
                    .to_string();
                sprites.push(SpriteEntry {
                    name,
                    x: 0,
                    y,
                    width: tile,
                    height: tile,
                });
                y += tile;
            }
        }
    }
    if sprites.is_empty() {
        sprites.push(SpriteEntry {
            name: "placeholder".into(),
            x: 0,
            y: 0,
            width: tile,
            height: tile,
        });
    }
    let manifest = AtlasManifest {
        image: "atlas.png".into(),
        width: tile,
        height: y.max(tile),
        sprites,
    };
    let manifest_path = output.join("atlas.json");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    println!(
        "Wrote {} ({} sprites). Placeholder MVP — add PNG sources to {}.",
        manifest_path.display(),
        manifest.sprites.len(),
        input.display()
    );
    Ok(())
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp"))
}
