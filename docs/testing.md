# Testing Guide

See [ROADMAP.md](ROADMAP.md) for tier priorities and planned test coverage.

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

| Area | Unit tests | Manual | Roadmap tier |
|------|------------|--------|--------------|
| Pathfinding / combat stats | server | combat in client | — |
| Tick loop / quests | server | tutorial quest | — |
| GE matching & escrow | server `economy.rs` | market panel | A2 |
| Dialogue / shop | server quest tests | guide NPC shop flow | — |
| Context menus / hover | engine | client | — |
| Content YAML | CI validator | — | — |
| Chat channel routing | server `social.rs` | Local/Global/Clan chat | A1 |
| Protocol serde | protocol `messages.rs` | — | A4 |
| Multi-region travel | server `state.rs`, region linter | walk through portal | A3 |
| WS integration | — (planned) | — | B3 |
| Postgres auth/persistence | — (manual) | login + reconnect | B1 |
| Redis sessions | — (manual) | multi-client session | B2 |
| Minigame / ledger | server `minigame.rs` | arena panel | C4 |
| Outlands skills | — (planned) | wrangling/ranching | C1 |

## Tier test expectations

### Tier A — Gameplay polish

**A1 Chat routing**

- `social.rs`: Local delivers only to same-region players; Global to all; Clan to clan members only
- Manual: switch channel in chat UI, verify message scope

**A2 GE escrow**

- `economy.rs`: `handle_market_offer` debits seller inventory for sell orders
- `economy.rs`: `match_offers` transfers escrowed items to buyer; cancel returns items
- Manual: place sell offer, verify inventory reduced; match or cancel

**A3 Multi-region**

- Region linter validates transition targets reference existing regions
- Server: walking onto transition tile moves player and sends `RegionChanged`
- Manual: walk portal between Verdant Reach and Shattered Coast

**A4 Protocol**

- Roundtrip JSON for every `ClientMessage` and `ServerMessage` variant

### Tier B — Production

**B3 WebSocket integration (planned)**

- Spawn server on ephemeral port; connect WS client; send walk/harvest intents; assert deltas
- CI job in `.github/workflows/ci.yml`

### Tier C — Content expansion

**C1 Outlands skills (planned)**

- Skills initialized at login; wrangling/ranching/engineering XP grants

## Optional features

- `postgres` + `DATABASE_URL` — account auth and character persistence
- `redis` + `REDIS_URL` — session cache (build with `--features redis`)
