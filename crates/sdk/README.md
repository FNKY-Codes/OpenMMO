# openmmo-sdk

Published schema contract for OpenMMO content packs. External tools and creator repos depend on this crate instead of vendoring the full engine monorepo.

## Contents

- Content type definitions (`ItemDef`, `RegionDef`, `QuestDef`, …)
- YAML content loader (`load_content`, `ContentPack`)
- Cross-reference validation (`validate_content`)
- Region geometry linting (`lint_regions`)
- Pack manifest parsing (`openmmo.toml`)

## Usage

```toml
[dependencies]
openmmo-sdk = "0.1"
```

```rust
use openmmo_sdk::{load_content, validate_content};
use std::path::Path;

let pack = load_content(Path::new("content"))?;
let errors = validate_content(&pack);
```
