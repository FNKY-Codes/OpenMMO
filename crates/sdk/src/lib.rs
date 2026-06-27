use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod content;
pub mod ids;
pub mod manifest;
pub mod math;
pub mod skills;
pub mod validation;

pub use content::*;
pub use ids::*;
pub use manifest::*;
pub use math::*;
pub use skills::*;
pub use validation::*;

/// Asset manifest entry for sprites, models, or audio files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetEntry {
    pub name: String,
    pub path: String,
    pub kind: AssetKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Sprite,
    Model,
    Audio,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetManifest {
    pub version: String,
    pub assets: Vec<AssetEntry>,
}

impl AssetManifest {
    pub fn scan(root: &Path, sprites: &Path, models: &Path, audio: &Path) -> anyhow::Result<Self> {
        let mut assets = Vec::new();
        collect_assets(&mut assets, root, sprites, AssetKind::Sprite)?;
        collect_assets(&mut assets, root, models, AssetKind::Model)?;
        collect_assets(&mut assets, root, audio, AssetKind::Audio)?;
        assets.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Self {
            version: "1".into(),
            assets,
        })
    }
}

fn collect_assets(
    out: &mut Vec<AssetEntry>,
    root: &Path,
    dir: &Path,
    kind: AssetKind,
) -> anyhow::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("asset")
                .to_string();
            let size_bytes = entry.metadata().ok().map(|m| m.len());
            out.push(AssetEntry {
                name,
                path: rel,
                kind,
                size_bytes,
            });
        }
    }
    Ok(())
}

/// Model metadata entry generated from 3D source files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub name: String,
    pub source: String,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    pub version: String,
    pub models: Vec<ModelMetadata>,
}

impl ModelManifest {
    pub fn scan(models_dir: &Path) -> anyhow::Result<Self> {
        let mut models = Vec::new();
        if models_dir.exists() {
            for entry in std::fs::read_dir(models_dir)? {
                let entry = entry?;
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let format = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase());
                let Some(fmt) = format else { continue };
                if !matches!(fmt.as_str(), "gltf" | "glb" | "obj") {
                    continue;
                }
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("model")
                    .to_string();
                let source = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                models.push(ModelMetadata {
                    name,
                    source,
                    format: fmt,
                    size_bytes: entry.metadata().ok().map(|m| m.len()),
                });
            }
        }
        models.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self {
            version: "1".into(),
            models,
        })
    }
}
