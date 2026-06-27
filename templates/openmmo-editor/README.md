# OpenMMO Editor

Visual authoring tools for OpenMMO content packs. This is a **separate repository** from the engine — clone or fork this template when building map, item, and NPC editors.

## Status

Scaffold only. Planned features:

- Map editor (tile painting, NPC/object spawn placement)
- Item and NPC definition editors
- Play-test button (launch local server + client against the open pack)

## Setup

```bash
git clone https://github.com/your-org/openmmo-editor.git
cd openmmo-editor
```

Add the SDK from crates.io (or a path dependency during development):

```toml
[dependencies]
openmmo-sdk = "0.1"
eframe = "0.30"
egui = "0.30"
```

## Development against the engine monorepo

When working inside the OpenMMO engine repo, use a path dependency:

```toml
openmmo-sdk = { path = "../../crates/sdk" }
```

## Project layout

```
openmmo-editor/
├── Cargo.toml
├── README.md
└── src/
    └── main.rs       # egui application entry point
```

## Writing content

Editors should read and write YAML under a content pack's `content/` directory using `openmmo-sdk` types. Validate output with:

```bash
cargo run -p openmmo-content-validator -- /path/to/pack/content
cargo run -p openmmo-region-linter -- /path/to/pack/content
```

See [OpenMMO content pack layout](https://github.com/fnky-codes/openmmo/blob/main/docs/content-pack.md) for the `openmmo.toml` manifest format.
