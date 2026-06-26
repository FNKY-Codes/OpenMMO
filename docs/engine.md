# Engine

## Stack

- **wgpu** — GPU rendering (desktop; wasm target planned)
- **winit** — window and input
- **egui** — inventory, bank, chat, minimap UI

## Coordinate System

3D world with tile logic on the XZ ground plane (Y is up):

- Each tile is a 1×1 world unit; tile `(x, y)` maps to world `(x, 0, y)`
- Orbit camera: right-drag to rotate, scroll to zoom, middle-drag to pan
- Left-click uses a screen ray → ground-plane intersection for walk intent

Legacy isometric helpers (`tile_to_screen`, `screen_to_tile`) remain in `openmmo-common` for tooling.

## Map Format

Regions defined in `content/regions/*.yaml`:

- `width`, `height`, `tiles` (byte per tile)
- `objects`, `npcs` spawn lists
- `spawn` point for new players

## Asset Pipeline

Phase 2+ includes `tools/atlas-packer` for sprite sheets. MVP uses colored geometry markers.
