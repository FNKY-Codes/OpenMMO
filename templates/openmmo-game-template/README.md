# My OpenMMO Game

Starter content pack for building your own OpenMMO shard.

## Quick start

1. Copy this template to a new repository (GitHub: **Use this template**).
2. Edit YAML files under `content/`.
3. Validate:

```bash
# From the OpenMMO engine repo (or install tools from crates.io when published)
cargo run -p openmmo-content-validator -- .
cargo run -p openmmo-region-linter -- .
cargo run -p openmmo-asset-manifest -- .
```

4. Run a server with your pack:

```bash
export OPENMMO_CONTENT=/path/to/this-repo/content
cargo run -p openmmo-server   # from the engine repo
```

## Layout

```
.
├── openmmo.toml      # Pack manifest
├── content/          # Game data (YAML)
│   ├── items/
│   ├── regions/
│   └── ...
└── assets/           # Sprites, models, audio (optional)
```

See [content pack documentation](https://github.com/fnky-codes/openmmo/blob/main/docs/content-pack.md) for details.
