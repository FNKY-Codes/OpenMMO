# OpenMMO

Open-source, OSRS-inspired tile-based MMO built with a custom Rust engine.

## Prerequisites

- Rust stable (`cargo`)
- Docker + Docker Compose (for PostgreSQL and Redis)
- GPU drivers for the desktop client (wgpu)

No Godot or third-party game editor required.

## Quick Start

```bash
# Start database services
docker compose -f deploy/docker-compose.yml up -d

# Validate game content
cargo run -p openmmo-content-validator -- content

# Run game server (port 8080)
cargo run -p openmmo-server

# Run desktop client (separate terminal)
cargo run -p openmmo-client
```

Connect the client to `ws://127.0.0.1:8080/ws`, enter a username and character name, then click **Connect**.

## Project Structure

```
crates/
  common/     Shared game types and content loading
  protocol/   WebSocket message definitions
  engine/     Custom wgpu + winit + egui renderer
  client/     Desktop game client
  server/     Authoritative tick-based game server
content/      YAML game data (items, NPCs, maps, quests)
tools/        Content validator, atlas packer
deploy/       Docker Compose and SQL migrations
docs/         Architecture and self-hosting guides
```

## Skills (Original Taxonomy)

14 consolidated skills in 4 Callings: Vitality, Prowess, Fortitude, Harvest, Refinement, Arcana, and more.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENMMO_CONTENT` | `content` | Path to content directory |
| `OPENMMO_BIND` | `0.0.0.0:8080` | Server bind address |
| `DATABASE_URL` | — | PostgreSQL connection (optional) |

## License

MIT
