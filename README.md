# OpenMMO

Open-source, post-apocalyptic tile-based MMO built with a custom Rust engine.

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
  sdk/        Published content schemas, validation, and pack manifest (openmmo-sdk)
  common/     Shared game types and content loading
  protocol/   WebSocket message definitions
  engine/     Custom wgpu + winit + egui renderer
  client/     Desktop game client
  server/     Authoritative tick-based game server
content/      YAML game data (items, NPCs, maps, quests) — reference pack
tools/        Content validator, region linter, asset manifest, model importer
templates/    Starter repos for creator content packs and visual editor
deploy/       Docker Compose and SQL migrations
docs/         Architecture, roadmap, and self-hosting guides
```

## Roadmap

See [docs/ROADMAP.md](docs/ROADMAP.md) for phase status, tier priorities, and planned features.

## Skills (Wasteland Taxonomy)

14 consolidated skills in 4 paths — Survival, Outlands, Workshop, and Operator — including Endurance, Combat, Resilience, Scavenging, Fabrication, Electrics, and more.

## Playing

Download the latest client from [Releases](https://github.com/FNKY-Codes/OpenMMO/releases)
— see [docs/PLAYING.md](docs/PLAYING.md).

## Deploying

The server ships as a Docker image (`Dockerfile`) and runs on Railway; see
[docs/self-hosting.md](docs/self-hosting.md) for variables and setup. Pushing a
`v*` tag builds and publishes the Windows client bundle.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `8080` | Server port |
| `OPENMMO_BIND` | `0.0.0.0:$PORT` | Server bind address (overrides `PORT`) |
| `OPENMMO_CONTENT` | `content` | Path to content directory |
| `OPENMMO_ASSETS` | `assets` | Path to model assets (client) |
| `OPENMMO_SERVER_URL` | `ws://127.0.0.1:8080/ws` | Default server URL on the client login screen |
| `DATABASE_URL` | — | PostgreSQL connection; required for persistence |
| `REDIS_URL` | — | Redis session cache (optional) |

## License

MIT
