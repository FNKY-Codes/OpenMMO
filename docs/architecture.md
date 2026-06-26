# Architecture

OpenMMO is a monorepo with a custom Rust engine client and authoritative tick-based server.

## Components

- **engine** — wgpu isometric renderer, winit input, egui UI overlays
- **client** — WebSocket networking + engine event loop
- **server** — 600ms tick loop, pathfinding, combat, skills, economy
- **protocol** — JSON-serialized messages over WebSocket
- **common** — shared types, content schemas, skill system

## Data Flow

```
Client (intent) → WebSocket → Server tick loop → StateDelta → Client render
```

All gameplay outcomes (XP, loot, movement) are resolved server-side.

## Tick Model

Default 600ms ticks (OSRS-inspired). Clients send intents; server resolves and broadcasts state.

## Persistence

PostgreSQL schema in `deploy/migrations/`. In-memory mode works without a database for local dev.
