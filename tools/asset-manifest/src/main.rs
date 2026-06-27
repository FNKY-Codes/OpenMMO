use std::fs;

use openmmo_sdk::{load_manifest, resolve_pack_root, AssetManifest};

fn main() -> anyhow::Result<()> {
    let arg = std::env::args().nth(1);
    let root = resolve_pack_root(arg.as_deref())?;
    let manifest = load_manifest(&root)?;

    let sprites = manifest.resolve_sprites_path(&root);
    let models = manifest.resolve_models_path(&root);
    let audio = manifest.resolve_audio_path(&root);
    let assets_root = manifest.resolve_assets_path(&root);

    let asset_manifest = AssetManifest::scan(&root, &sprites, &models, &audio)?;
    fs::create_dir_all(&assets_root)?;

    let output = assets_root.join("manifest.json");
    fs::write(&output, serde_json::to_string_pretty(&asset_manifest)?)?;
    println!(
        "Wrote {} ({} assets) for pack {:?}",
        output.display(),
        asset_manifest.assets.len(),
        manifest.pack.name
    );
    Ok(())
}
