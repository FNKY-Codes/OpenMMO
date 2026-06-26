use std::collections::HashMap;
use std::path::Path;

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
