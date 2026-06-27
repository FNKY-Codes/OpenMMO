use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackManifest {
    pub pack: PackInfo,
    pub engine: EngineRequirement,
    #[serde(default)]
    pub paths: PackPaths,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackInfo {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineRequirement {
    pub min_sdk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackPaths {
    #[serde(default = "default_content")]
    pub content: String,
    #[serde(default = "default_assets")]
    pub assets: String,
    #[serde(default = "default_sprites")]
    pub sprites: String,
    #[serde(default = "default_models")]
    pub models: String,
    #[serde(default = "default_audio")]
    pub audio: String,
    #[serde(default = "default_atlas")]
    pub atlas: String,
}

impl Default for PackPaths {
    fn default() -> Self {
        Self {
            content: default_content(),
            assets: default_assets(),
            sprites: default_sprites(),
            models: default_models(),
            audio: default_audio(),
            atlas: default_atlas(),
        }
    }
}

fn default_content() -> String {
    "content".into()
}
fn default_assets() -> String {
    "assets".into()
}
fn default_sprites() -> String {
    "assets/sprites".into()
}
fn default_models() -> String {
    "assets/models".into()
}
fn default_audio() -> String {
    "assets/audio".into()
}
fn default_atlas() -> String {
    "assets/atlas".into()
}

impl PackManifest {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let manifest: PackManifest = toml::from_str(&data)?;
        Ok(manifest)
    }

    pub fn resolve_content_path(&self, manifest_dir: &Path) -> PathBuf {
        manifest_dir.join(&self.paths.content)
    }

    pub fn resolve_assets_path(&self, manifest_dir: &Path) -> PathBuf {
        manifest_dir.join(&self.paths.assets)
    }

    pub fn resolve_sprites_path(&self, manifest_dir: &Path) -> PathBuf {
        manifest_dir.join(&self.paths.sprites)
    }

    pub fn resolve_models_path(&self, manifest_dir: &Path) -> PathBuf {
        manifest_dir.join(&self.paths.models)
    }

    pub fn resolve_audio_path(&self, manifest_dir: &Path) -> PathBuf {
        manifest_dir.join(&self.paths.audio)
    }

    pub fn resolve_atlas_path(&self, manifest_dir: &Path) -> PathBuf {
        manifest_dir.join(&self.paths.atlas)
    }
}

/// Load `openmmo.toml` from `dir`, or return defaults if missing.
pub fn load_manifest(dir: &Path) -> anyhow::Result<PackManifest> {
    let manifest_path = dir.join("openmmo.toml");
    if manifest_path.exists() {
        PackManifest::load(&manifest_path)
    } else {
        Ok(PackManifest {
            pack: PackInfo {
                name: dir
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("content")
                    .to_string(),
                version: "0.0.0".into(),
                description: None,
                author: None,
            },
            engine: EngineRequirement {
                min_sdk: "0.1.0".into(),
            },
            paths: PackPaths::default(),
        })
    }
}

/// Resolve content directory from CLI arg, `OPENMMO_CONTENT`, or `openmmo.toml`.
pub fn resolve_content_path(arg: Option<&str>) -> anyhow::Result<PathBuf> {
    if let Some(path) = arg {
        let path = PathBuf::from(path);
        if path.join("openmmo.toml").exists() {
            let manifest = load_manifest(&path)?;
            return Ok(manifest.resolve_content_path(&path));
        }
        return Ok(path);
    }

    if let Ok(env_path) = std::env::var("OPENMMO_CONTENT") {
        return Ok(PathBuf::from(env_path));
    }

    let cwd = std::env::current_dir()?;
    if cwd.join("openmmo.toml").exists() {
        let manifest = load_manifest(&cwd)?;
        return Ok(manifest.resolve_content_path(&cwd));
    }

    Ok(PathBuf::from("content"))
}

/// Resolve pack root (directory containing openmmo.toml or content/).
pub fn resolve_pack_root(arg: Option<&str>) -> anyhow::Result<PathBuf> {
    if let Some(path) = arg {
        let path = PathBuf::from(path);
        if path.join("openmmo.toml").exists() {
            return Ok(path);
        }
        if path.ends_with("content") {
            return Ok(path.parent().unwrap_or(&path).to_path_buf());
        }
        return Ok(path);
    }

    let cwd = std::env::current_dir()?;
    if cwd.join("openmmo.toml").exists() {
        return Ok(cwd);
    }

    Ok(cwd)
}
