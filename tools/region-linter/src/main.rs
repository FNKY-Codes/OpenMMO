use openmmo_sdk::{lint_regions, load_content, resolve_content_path};

fn main() -> anyhow::Result<()> {
    let arg = std::env::args().nth(1);
    let path = resolve_content_path(arg.as_deref())?;
    let pack = load_content(&path)?;
    let errors = lint_regions(&pack);

    if errors.is_empty() {
        println!(
            "OK: {} region(s) passed geometry checks",
            pack.regions.len()
        );
        Ok(())
    } else {
        for e in &errors {
            eprintln!("ERROR: {e}");
        }
        anyhow::bail!("{} region lint errors", errors.len())
    }
}
