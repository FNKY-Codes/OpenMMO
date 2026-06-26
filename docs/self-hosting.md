# Self-Hosting

## Requirements

- Linux server with Rust toolchain (or pre-built binaries)
- Docker for PostgreSQL and Redis
- Open port 8080 (WebSocket)

## Steps

1. Clone the repository
2. Start infrastructure:

```bash
docker compose -f deploy/docker-compose.yml up -d
```

3. Set environment:

```bash
export DATABASE_URL=postgres://openmmo:openmmo@localhost/openmmo
export OPENMMO_CONTENT=content
export OPENMMO_BIND=0.0.0.0:8080
```

4. Build and run:

```bash
cargo build --release -p openmmo-server
./target/release/openmmo-server
```

5. Distribute client builds or build locally:

```bash
cargo build --release -p openmmo-client
```

## Custom Content

Replace or extend files in `content/`. Run the validator before deploying.

## Backups

Back up the PostgreSQL volume (`pgdata`) and your `content/` directory.

## Plugin API (v0.1)

Server-side plugin hooks are stubbed in `crates/server/src/anticheat.rs` (`PluginApi::on_tick`). WASM/Lua sandbox planned for future releases.

## Moderation

Send `moderator_command` messages (requires moderator flag — configure per shard).

Commands: kick, ban, teleport, spawn_item.

## Anti-Cheat

Server-authoritative movement, XP, and loot. Audit log in server memory (persist to DB in production).
