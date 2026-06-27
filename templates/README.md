# OpenMMO Templates

Starter scaffolds for third-party MMO creators. These are meant to be copied into **separate repositories**, not used directly from the engine monorepo.

| Template | Purpose |
|----------|---------|
| [`openmmo-game-template/`](openmmo-game-template/) | Starter content pack with `openmmo.toml`, skeleton YAML, and CI validation workflow |
| [`openmmo-editor/`](openmmo-editor/) | Scaffold for the visual editor repo (depends on `openmmo-sdk`) |

## Publishing as GitHub templates

1. Create a new repository from each template directory.
2. Enable **Template repository** in GitHub repository settings.
3. Creators use **Use this template** to start their own content pack or editor project.

## Related docs

- [Content pack layout](../docs/content-pack.md)
- [Content authoring](../docs/content-authoring.md)
