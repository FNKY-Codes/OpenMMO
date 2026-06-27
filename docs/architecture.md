# Architecture

OpenMMO is a monorepo with a custom Rust engine client and authoritative tick-based server.

## Components

- **engine** — wgpu isometric renderer, winit input, egui UI overlays
- **client** — WebSocket networking + engine event loop
- **server** — 600ms tick loop, pathfinding, combat, skills, economy
- **protocol** — JSON-serialized messages over WebSocket
- **common** — shared types, content schemas, skill system
- **sdk** (`openmmo-sdk`) — publishable content schemas, validation, and pack manifest for third-party creators

## Data Flow

```
Client (intent) → WebSocket → Server tick loop → StateDelta / WorldSnapshot → Client render
```

All gameplay outcomes (XP, loot, movement) are resolved server-side.

Private state (inventory, skills, quests, trade, market, ledger) is routed to the initiating player only. Shared world state (entity positions, combat visuals) uses broadcast deltas.

On join, clients receive a full `WorldSnapshot`. Thereafter the server sends `StateDelta` each tick for changed entities, with a periodic full snapshot as an anti-desync safety net.

## Tick Model

Default 600ms ticks (OSRS-inspired). Clients send intents; the server resolves outcomes and emits targeted or broadcast messages.

## Persistence

PostgreSQL schema in `deploy/migrations/`. Build with `--features postgres` and set `DATABASE_URL` for character save/load on disconnect and audit log persistence. In-memory mode works without a database for local dev.
