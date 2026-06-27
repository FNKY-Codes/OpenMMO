use openmmo_sdk::{load_content, resolve_content_path, validate_content};

fn main() -> anyhow::Result<()> {
    let arg = std::env::args().nth(1);
    let path = resolve_content_path(arg.as_deref())?;
    let pack = load_content(&path)?;
    let errors = validate_content(&pack);

    if errors.is_empty() {
        println!(
            "OK: {} items, {} npcs, {} regions, {} quests, {} skills",
            pack.items.len(),
            pack.npcs.len(),
            pack.regions.len(),
            pack.quests.len(),
            pack.skills.len()
        );
        Ok(())
    } else {
        for e in &errors {
            eprintln!("ERROR: {e}");
        }
        anyhow::bail!("{} validation errors", errors.len())
    }
}
