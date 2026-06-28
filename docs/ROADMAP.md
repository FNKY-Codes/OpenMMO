# OpenMMO Roadmap

**Last reviewed:** 2026-06-27

## Vision & principles

OpenMMO is an open-source, post-apocalyptic tile-based MMO built with a custom Rust engine. Core design goals:

- **Server-authoritative** — all gameplay outcomes (XP, loot, movement, economy) resolve on the tick loop
- **YAML-driven content** — items, NPCs, regions, quests, and skills ship as data, not code
- **Creator-friendly SDK** — third-party content packs validate against `openmmo-sdk` schemas

See [architecture.md](architecture.md) for component layout and data flow.

## Status summary

| Phase | Goal | Status |
|-------|------|--------|
| Phase 0 — Foundation | Workspace, engine, protocol, infra, CI | **Complete** |
| Phase 1 — Core loop MVP | Skills, harvest/refine/combat, inventory/bank/chat | **Complete** |
| Phase 2 — Content pipeline | YAML content, validator, tutorial quest | **Complete** |
| Phase 3 — Economy & social | GE, trading, friends, ledger, minigames | **Mostly complete** |
| Phase 4 — Hardening | Anti-cheat, auth, persistence, docs | **Partial** — auth/redis added; postgres opt-in |
| Phase 5 — Tooling | Atlas pipeline, WASM, plugins, editor | **Partial** — stubs expanded |

## Recent fixes (2026-06-27)

- **Shop:** `DialogueSelect` sends `dialogue_id` so nested dialogue opens shop correctly
- **XP:** Combat/harvest/refine sync `SkillUpdate` + `XpDrop`; HUD floaters persist 1.5s
- **Combat timing:** `attack_ticks` on weapons and NPCs; cooldown-based swings (no random coin flip)
- **Hover highlight:** Accent outline on hovered entity/tile in 3D view
- **GE:** Fixed offer removal index bug when matching buy/sell orders
- **Spells UI:** Cast spells from Skills tab when in combat
- **Auth:** Password + account table when `DATABASE_URL` + `--features postgres`
- **Redis:** Session cache when `REDIS_URL` + `--features redis`
- **Content:** Fishing rod, second region (`shattered_coast`), functional minimap dot

## Test matrix

See [testing.md](testing.md). CI runs `cargo test --workspace`, fmt, clippy, content-validator, region-linter.

---

## Tier A — Gameplay polish (highest priority)

Close functional gaps in shipped systems. Low architectural risk; high player-facing impact.

| ID | Item | Status | Key files |
|----|------|--------|-----------|
| A1 | Chat channel routing (Local/Global/Clan) | **Complete** | `crates/server/src/social.rs`, `crates/engine/src/ui/mod.rs` |
| A2 | Grand Exchange seller escrow on match | **Complete** | `crates/server/src/economy.rs` |
| A3 | Multi-region travel | **Complete** | `crates/sdk/src/content.rs`, `crates/server/src/state.rs`, `content/regions/` |
| A4 | Full protocol serde tests | **Complete** | `crates/protocol/src/messages.rs` |

### A1. Chat channel routing

**Problem:** Protocol defines `Local`, `Global`, `Clan`, `Private`, but `broadcast_chat()` does not filter recipients. Client always sends `ChatChannel::Local`.

**Deliverables:**

- Server: Local = same region, Global = all online, Clan = `clan_members` lookup
- Client: channel picker in chat UI
- Unit tests in `social.rs`

**Depends on:** nothing (A3 improves Local accuracy once regions are wired)

### A2. Grand Exchange seller inventory

**Problem:** Sell offers debit inventory at placement, but matching lacked explicit escrow validation and end-to-end test coverage.

**Deliverables:**

- Escrow tracked via offer `quantity` for sell orders
- Unit test: place sell offer → inventory debited → match → buyer receives items
- Unit test: cancel returns escrowed items

**Depends on:** nothing

### A3. Multi-region travel

**Problem:** `shattered_coast` region exists; players cannot transition between regions.

**Deliverables:**

- `RegionTransition` in region YAML schema
- Server: per-region walkable tiles, entity tagging, transition on walk
- Protocol: `RegionChanged` message with region-scoped entity snapshot
- Client: update active region and entity list on transition
- Region linter validates transition targets

**Depends on:** nothing

### A4. Full protocol serde tests

**Deliverables:** Roundtrip JSON encode/decode for every `ClientMessage` and `ServerMessage` variant.

**Depends on:** nothing

---

## Tier B — Production hardening

Required before public self-hosting or multi-shard deployment.

| ID | Item | Status | Key files |
|----|------|--------|-----------|
| B1 | Postgres default in release profile | Open | `crates/server/Cargo.toml`, `docs/self-hosting.md` |
| B2 | Redis sessions in production path | Open | `crates/server/src/session.rs`, `deploy/docker-compose.yml` |
| B3 | WebSocket integration tests | Open | `.github/workflows/ci.yml` |
| B4 | Audit log persistence | Open | `deploy/migrations/`, `crates/server/src/persistence.rs` |
| B5 | CI release workflow | Open | `.github/workflows/` |

**Depends on:** B3 after A4; B4 after B1; B5 after B1 + B2

---

## Tier C — Content & gameplay expansion

| ID | Item | Status | Key files |
|----|------|--------|-----------|
| C1 | Outlands skills (Wrangling, Ranching, Engineering) | Open | `content/skills/`, `crates/server/src/state.rs` |
| C2 | Fishing skill loop | Open | `content/items/fishing_rod.yaml`, `content/objects/` |
| C3 | Clan system (create/join/leave) | Open | `crates/server/src/social.rs`, protocol |
| C4 | Minigame & ledger depth | Open | `crates/server/src/minigame.rs` |
| C5 | Additional regions & quest chains | Open | `content/regions/`, `content/quests/` |

**Depends on:** C1 and C5 after A3; C3 after A1 + B1

---

## Tier D — Rendering, platform, and creator tools (Phase 5)

| ID | Item | Status | Key files |
|----|------|--------|-----------|
| D1 | Sprite atlas pipeline (PNG packing) | Open | `tools/atlas-packer/`, `crates/engine/src/renderer.rs` |
| D2 | GLB model rendering improvements | Open | `crates/engine/src/model.rs`, `assets/models/` |
| D3 | WASM browser client | Open | `crates/engine/src/lib.rs` |
| D4 | Plugin sandbox (WASM/Lua) | Open | `crates/server/src/anticheat.rs`, `crates/sdk/src/manifest.rs` |
| D5 | Visual editor | Open | `templates/openmmo-editor/` |

**Depends on:** D3 after B3; D4 after B1; D5 optionally after D1

---

## Creator ecosystem (parallel track)

| Item | Path | Work |
|------|------|------|
| Content pack template | `templates/openmmo-game-template/` | Expand examples, publish to crates.io |
| SDK publishing | `crates/sdk/` | Versioned releases, changelog |
| CONTRIBUTING.md | new | Dev setup, PR guidelines, roadmap link |
| Issue templates | `.github/ISSUE_TEMPLATE/` | Bug, feature, content pack |

---

## Known gaps register

| Gap | File(s) | Tier | Notes |
|-----|---------|------|-------|
| Chat routing | `social.rs`, `app.rs` | A1 | Done |
| GE seller escrow | `economy.rs` | A2 | Done |
| Multi-region travel | `state.rs`, region YAML | A3 | Done |
| Outlands skills not initialized | `state.rs`, skill YAML | C1 | YAML ahead of gameplay |
| Postgres/redis opt-in | `server/Cargo.toml` | B1–B2 | Requires `--features` |
| Atlas PNG packing | `tools/atlas-packer` | D1 | Manifest-only MVP |
| WASM client | `engine/src/lib.rs` | D3 | `wasm_main` stub |
| Plugin sandbox | `anticheat.rs` | D4 | `on_tick` hook only |
| Visual editor | `templates/openmmo-editor` | D5 | Scaffold only |

---

## Dependency graph

```mermaid
flowchart TD
  A1[A1_ChatRouting]
  A2[A2_GE_Escrow]
  A3[A3_MultiRegion]
  A4[A4_ProtocolTests]
  B1[B1_PostgresDefault]
  B3[B3_WSIntegrationTests]
  C1[C1_OutlandsSkills]
  C3[C3_ClanSystem]
  D3[D3_WASMClient]
  A4 --> B3
  A1 --> C3
  A3 --> C1
  B1 --> C3
  B3 --> D3
```

### Suggested release themes

1. **Polish release** — A1, A2, A4
2. **World expansion** — A3, C2, C5
3. **Production release** — B1, B2, B3, B4, B5
4. **Outlands release** — C1, C3, C4
5. **Platform release** — D1, D2, D3
6. **Creator release** — D4, D5, ecosystem docs

---

## Completed tier items

### Tier A (prior)

- [x] Spell casting UI (Skills tab → `CastSpell`)
- [x] GE unit tests + index fix
- [x] Speed-hack detection on movement steps
- [x] Chat channel routing (Local/Global/Clan)
- [x] Full protocol serde tests
- [x] Multi-region travel (Verdant Reach ↔ Shattered Coast)
- [x] GE seller escrow end-to-end tests

### Tier B (prior)

- [x] Password auth (postgres feature)
- [x] Redis session role defined

### Tier C (prior)

- [x] Second region YAML (`shattered_coast`)
- [x] BETA skills initialized at login (`state.rs`)
- [x] Specialization picker UI
- [x] Collection log client handler
- [x] Minimap player position
- [x] Fishing rod item + shop stock

---

## How to update

When closing roadmap items:

1. Update the status table and checkboxes in this file
2. Add or update test expectations in [testing.md](testing.md)
3. Move completed items to the "Completed tier items" section
