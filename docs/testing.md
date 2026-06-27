# Testing Guide

## Automated tests

```bash
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Content validation

```bash
cargo run -p openmmo-content-validator -- content
cargo run -p openmmo-region-linter -- content
```

## Manual smoke test

```bash
docker compose -f deploy/docker-compose.yml up -d
./scripts/start-game.sh
```

Connect to `ws://127.0.0.1:8080/ws`. Exercise: harvest, refine, combat (XP floaters), nested dialogue shop, GE offers, arena.

## Test matrix

| Area | Unit tests | Manual |
|------|------------|--------|
| Pathfinding / combat stats | server | combat in client |
| Tick loop / quests | server | tutorial quest |
| GE matching | server economy tests | market panel |
| Dialogue / shop | server quest tests | guide NPC shop flow |
| Context menus / hover | engine | client |
| Content YAML | CI validator | — |

## Optional features

- `postgres` + `DATABASE_URL` — account auth and character persistence
- `redis` + `REDIS_URL` — session cache (build with `--features redis`)
