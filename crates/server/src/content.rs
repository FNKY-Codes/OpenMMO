use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use openmmo_common::ContentPack;

pub fn load_content_dir(path: &Path) -> anyhow::Result<ContentPack> {
    openmmo_common::load_content(path)
}

pub fn default_content_path() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("OPENMMO_CONTENT").unwrap_or_else(|_| "content".to_string()),
    )
}

pub struct ContentRegistry {
    pub pack: ContentPack,
    pub item_index: HashMap<u32, usize>,
}

impl ContentRegistry {
    pub fn new(pack: ContentPack) -> Self {
        let item_index = pack
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| (item.id.0, i))
            .collect();
        Self { pack, item_index }
    }
}

/// Reload YAML content from disk when the content directory mtime changes.
pub fn maybe_reload(
    path: &Path,
    last_mtime: &mut Option<SystemTime>,
    world: &mut crate::state::GameWorld,
) -> anyhow::Result<bool> {
    let mtime = dir_mtime(path)?;
    if last_mtime.is_some_and(|t| t >= mtime) {
        return Ok(false);
    }
    let content = load_content_dir(path)?;
    world.content = content;
    world.quests.load(&world.content.clone());
    *last_mtime = Some(mtime);
    tracing::info!("Hot-reloaded content from {}", path.display());
    Ok(true)
}

fn dir_mtime(path: &Path) -> anyhow::Result<SystemTime> {
    let mut latest = SystemTime::UNIX_EPOCH;
    if !path.exists() {
        return Ok(latest);
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if let Ok(modified) = meta.modified() {
            if modified > latest {
                latest = modified;
            }
        }
    }
    Ok(latest)
}

pub fn spawn_hot_reload_task(world: crate::state::SharedWorld) {
    let path = default_content_path();
    tokio::spawn(async move {
        let mut last_mtime = None;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            let mut w = world.write().await;
            if let Err(e) = maybe_reload(&path, &mut last_mtime, &mut w) {
                tracing::warn!("Content hot-reload failed: {e}");
            }
        }
    });
}
