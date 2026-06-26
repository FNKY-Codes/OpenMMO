# Engine

## Stack

- **wgpu** — GPU rendering (desktop; wasm target planned)
- **winit** — window and input
- **egui** — inventory, bank, chat, minimap UI

## Coordinate System

Isometric tile projection:

- `tile_to_screen` — world tile → screen pixels
- `screen_to_tile` — click position → tile (point-and-click)

## Map Format

Regions defined in `content/regions/*.yaml`:

- `width`, `height`, `tiles` (byte per tile)
- `objects`, `npcs` spawn lists
- `spawn` point for new players

## Asset Pipeline

Phase 2+ includes `tools/atlas-packer` for sprite sheets. MVP uses colored geometry markers.
