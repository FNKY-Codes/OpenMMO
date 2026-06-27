use std::fs;

use openmmo_sdk::{load_manifest, resolve_pack_root, ModelManifest};

fn main() -> anyhow::Result<()> {
    let arg = std::env::args().nth(1);
    let root = resolve_pack_root(arg.as_deref())?;
    let manifest = load_manifest(&root)?;
    let models_dir = manifest.resolve_models_path(&root);

    let model_manifest = ModelManifest::scan(&models_dir)?;
    fs::create_dir_all(&models_dir)?;

    let output = models_dir.join("manifest.yaml");
    fs::write(&output, serde_yaml::to_string(&model_manifest)?)?;
    println!(
        "Wrote {} ({} models) for pack {:?}",
        output.display(),
        model_manifest.models.len(),
        manifest.pack.name
    );
    Ok(())
}
