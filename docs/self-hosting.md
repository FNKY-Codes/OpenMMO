# Self-Hosting

The official server runs on Railway; this page covers running your own.

## What you need

- A Linux host with Docker (or the Rust toolchain to build natively)
- PostgreSQL 16 — required for accounts and character saves
- Redis 7 — optional session cache
- A public HTTPS/WSS endpoint (Railway/Fly/Caddy/nginx terminate TLS; the
  server itself speaks plain HTTP + WebSocket)

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `8080` | Port to listen on (PaaS convention) |
| `OPENMMO_BIND` | `0.0.0.0:$PORT` | Full bind address; overrides `PORT` |
| `OPENMMO_CONTENT` | `content` | Path to the YAML content directory |
| `DATABASE_URL` | — | PostgreSQL URL. **Without it the server is memory-only and nothing persists.** |
| `REDIS_URL` | — | Redis URL for the session cache (optional) |
| `RUST_LOG` | `openmmo_server=info` | Log filter |

The server applies its own schema on startup (`CREATE TABLE IF NOT EXISTS`),
so an empty database is enough.

## Docker (recommended)

The repo's `Dockerfile` builds the server with the `postgres,redis` features and
bakes in `content/`.

```bash
docker build -t openmmo-server .
docker run -p 8080:8080 \
  -e DATABASE_URL=postgres://openmmo:openmmo@db/openmmo \
  -e REDIS_URL=redis://cache:6379 \
  openmmo-server
```

For local infra, `deploy/docker-compose.yml` starts Postgres and Redis.

## Railway

1. Create a project; add **Postgres** and **Redis** services (or any image with
   a volume on `/var/lib/postgresql/data`).
2. Add a service from this GitHub repo. Railway detects the `Dockerfile`.
3. Set variables on the server service:
   - `DATABASE_URL` = `${{Postgres.DATABASE_URL}}`
   - `REDIS_URL` = `${{Redis.REDIS_URL}}`
4. Generate a public domain. Players connect to `wss://<domain>/ws`.
5. Set the healthcheck path to `/health`.

## Native build

```bash
cargo build --release -p openmmo-server --features postgres,redis
DATABASE_URL=postgres://... ./target/release/openmmo-server
```

## Persistence & shutdown

- Characters are saved every 60 s, on logout, and when the process receives
  `SIGTERM`/`Ctrl-C` (Railway sends `SIGTERM` on redeploy).
- Passwords are stored as Argon2id hashes. The first login with a new username
  creates the account; a character belongs to the account that created it.
- Logging in with a character that is already online disconnects the older
  session.

## Content

Edit files in `content/`; the server hot-reloads the directory every 5 s. Run
the validator before shipping:

```bash
cargo run -p openmmo-content-validator -- content
cargo run -p openmmo-region-linter -- content
```

The client also bundles `content/` — keep server and client content in sync
(the release workflow packages whatever is in the repo at the tag).

## Backups

Back up the PostgreSQL volume and your `content/` directory.

## Client distribution

Push a tag `vX.Y.Z` to build `openmmo-client-vX.Y.Z-windows-x64.zip` and attach
it to a GitHub Release. Set the `OPENMMO_SERVER_URL` repository variable to your
`wss://…/ws` URL so the login screen defaults to your server.

## Moderation

Send `moderator_command` messages (requires the `is_moderator` flag on the
character). Commands: kick, ban, teleport, spawn_item.

## Plugin API (v0.1)

Server-side plugin hooks are stubbed in `crates/server/src/anticheat.rs`
(`PluginApi::on_tick`). WASM/Lua sandbox planned.
