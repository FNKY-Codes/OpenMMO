# OpenMMO Roadmap

This document tracks planned work, current delivery status, and recommended priorities for OpenMMO. It reflects the phased plan from the v0.1 foundation release, updated against what is actually shipped.

**Last reviewed:** 2026-06-26 (roadmap implementation complete)

## Status Summary

| Phase | Goal | Status |
|-------|------|--------|
| Phase 0 — Foundation | Workspace, engine, protocol, infra, CI | **Complete** |
| Phase 1 — Core loop MVP | Skills, harvest/refine/combat, inventory/bank/chat | **Complete** — client wired end-to-end |
| Phase 2 — Content pipeline | YAML content, validator, tutorial quest | **Complete** — 14 skills, guide NPC, quest journal |
| Phase 3 — Economy & social | GE, trading, friends, ledger, minigames | **Mostly complete** — settlement, shops, per-player routing |
| Phase 4 — Hardening | Anti-cheat, mod tools, plugin API, docs | **Partial** — speed-hack, mod gate, optional PostgreSQL |
| Phase 5 — Tooling | Atlas pipeline, hot-reload, WASM, plugins | **Partial** — stubs and hooks in place |

**Overall:** Core gameplay loop is playable through the client. Economy and social systems are wired server-side and in the HUD. Production persistence and sprite rendering remain optional/future work.

---

## Phase 0 — Foundation ✅

**Goal:** Establish the monorepo, custom Rust engine, authoritative tick server, and developer tooling.

### Delivered

- Rust workspace: `common`, `protocol`, `engine`, `client`, `server`
- Custom **wgpu + winit + egui** engine
- WebSocket JSON protocol (login, movement, interact, inventory, chat)
- Docker Compose (PostgreSQL + Redis)
- GitHub Actions CI (`fmt`, `clippy`, `build`, `test`, content validation)

### Success criteria

- [x] `cargo build --workspace` passes
- [x] `cargo test --workspace` passes
- [x] Content validator runs in CI
- [x] Server health check responds at `/health`

---

## Phase 1 — Core Loop MVP ✅

**Goal:** A playable skilling and combat loop — harvest, refine, fight, bank, chat.

### Delivered

- 600ms authoritative tick loop with A* pathfinding
- Harvest/refine timers with XP grants; melee and spell combat
- Inventory (28 slots), bank, ground items, equipment equipping
- Entity picking and click-to-interact (harvest, attack, pickup, talk)
- HUD: bank deposit/withdraw, refine recipes, dialogue, shop, combat/XP feedback
- Per-player `StateDelta` updates with periodic full `WorldSnapshot`
- Equipment `prowess_bonus` applied in combat; tool tags enforced for harvest
- NPC aggro and pursuit when `aggro_range > 0`

### Remaining polish

- Spells always hit (no accuracy roll)
- Right-click context menus not implemented

### Success criteria

- [x] Player can harvest, refine, fight, and bank through normal client interactions
- [x] XP, damage, and skill updates appear in the HUD without manual protocol messages
- [x] Equipment affects combat stats
- [x] Tool tags are enforced for harvest nodes

---

## Phase 2 — Content Pipeline ✅

**Goal:** Data-driven game content with validation and a tutorial onboarding path.

### Delivered

- YAML content in `content/` (items, NPCs, objects, regions, quests, dialogues, skills, specializations, recipes, spells, shops)
- `content-validator` with loot, recipe, quest, dialogue, shop, and region checks
- Tutorial quest *First Steps in the Reach* with guide NPC (`npc_id: 100`)
- Quest counters, all objective types (`TalkToNpc`, `ReachSkillLevel`, `VisitTile`, etc.)
- `QuestJournal` sent on login; `QuestUpdate` during play
- All 14 skills have YAML definitions
- `tools/atlas-packer` manifest stub; colored geometry MVP rendering
- Content hot-reload via file watcher when `CONTENT_PATH` is set

### Success criteria

- [x] Tutorial quest is completable as authored
- [x] All 14 skills have content definitions
- [x] Content validator covers dialogues, shops, and regions
- [x] Quest journal is sent to client on login and updates during play
- [x] `atlas-packer` tool exists (manifest stub; sprite pipeline is future work)

---

## Phase 3 — Economy & Social ⚠️

**Goal:** Player-to-player economy, social features, and instanced content.

### Delivered

| Feature | Protocol | Server logic | Client UI | Settlement |
|---------|----------|--------------|-----------|------------|
| Grand Exchange | Yes | Offer matching + transfer | Market panel wired | Item/currency transfer |
| Player trading | Yes | Bilateral accept + swap | Friends panel wired | Both sides receive items |
| Friends / PMs | Yes | Friend add + PM routing | Friends panel wired | Per-recipient PM delivery |
| Ledger contracts | Yes | Kill progress tracking | Status in HUD | Per-player updates |
| Arena minigame | Yes | Boss spawn + phases | Join button wired | Boss in snapshots + combat |
| Shops | Yes | `ShopBuy` handler | Shop modal from dialogue | Currency debit + item grant |

### Remaining polish

- Trade offer UI (adding items to trade window) not yet in HUD — accept/request only
- GE matching edge cases need more playtesting
- Second region beyond `verdant_reach.yaml`

### Success criteria

- [x] GE offers match and transfer items/currency to players
- [x] Player trade completes with item swap on both sides
- [x] PMs route to the intended recipient
- [x] Shop buying works from `content/shops/`
- [x] Arena minigame is joinable and the boss is damageable/rendered
- [x] Ledger updates are per-player, not global broadcast

---

## Phase 4 — Hardening ⚠️

**Goal:** Production readiness for self-hosted shards.

### Delivered

- Architecture, engine, self-hosting, and content-authoring docs
- Moderator commands with `is_moderator` permission gate
- `detect_speed_hack()` on movement; username format validation on login
- PostgreSQL save/load on disconnect (`--features postgres` + `DATABASE_URL`)
- Audit log persisted to DB when `DATABASE_URL` is set
- Plugin API hook point (`PluginApi::on_tick`)

### Gaps

- No password/token authentication (username format check only)
- `postgres` feature not enabled by default in release builds
- Redis is in Docker Compose but unused in Rust code
- `PluginApi::on_tick` is a stub — WASM/Lua sandbox is future work

### Success criteria

- [x] Characters persist across disconnect/reconnect via PostgreSQL (when feature enabled)
- [x] Account auth validates username format
- [x] Speed-hack detection runs on movement
- [x] Moderator commands require a permission flag
- [x] Audit log persists to DB in production deployments (when `DATABASE_URL` set)

---

## Phase 5 — Engine & Tooling (Future)

**Goal:** Asset pipeline, live content iteration, browser client, plugin ecosystem.

### Delivered (stubs)

- `tools/atlas-packer` — JSON manifest generator (no sprite loading in engine yet)
- Content hot-reload file watcher in server
- WASM client library stub in `crates/engine/src/lib.rs`
- `PluginApi::on_tick` no-op hook

### Remaining

- Engine loads packed sprite atlases instead of colored blocks
- WASM browser client target
- WASM/Lua plugin sandbox

---

## Post-Foundation Work

| Change | Status | PR |
|--------|--------|-----|
| 3D world rendering (colored blocks on XZ plane) | Done | #3 |
| Orbit camera (right-drag rotate, scroll zoom) | Done | #3 |
| Camera locked on local player while moving | Done | #4, #5 |
| Roadmap implementation (M1–M6) | Done | #8 |

---

## Recommended Priority Stack

### Tier 1 — Make it playable ✅

1. Wire client interactions: harvest, attack, pickup, refine, bank deposit/withdraw
2. Handle server→client messages for XP, damage, quest progress, and dialogue
3. Fix quest system: objective types, progress counters, send journal on login
4. Fix quest content bug (guide NPC vs Meadow Crawler on `npc_id: 1`)
5. Per-player WebSocket routing instead of global broadcast

### Tier 2 — Complete the vertical slice ✅

6. Trade and GE item/currency settlement
7. Equipment equipping and combat stat application
8. Shop buying from `content/shops/`
9. NPC aggro and pursuit
10. Expand content-validator (dialogues, regions, shops, spell refs)

### Tier 3 — Persistence and ops (partial)

11. Character save/load via PostgreSQL — **done** (opt-in feature)
12. Account authentication — **username validation only**
13. Wire up anti-cheat — **done**
14. Persist audit log to DB — **done** (when `DATABASE_URL` set)
15. Define and implement Redis usage — **not started**

### Tier 4 — Future

16. `tools/atlas-packer` sprite pipeline and engine atlas loading
17. WASM browser client target
18. WASM/Lua plugin sandbox
19. Second region and expanded world content

---

## Known Doc/Code Inconsistencies

| Claim | Reality |
|-------|---------|
| README implies full sprite atlas pipeline | `atlas-packer` emits manifest JSON only; engine still uses colored geometry |
| Redis in prerequisites and Docker Compose | Zero references in Rust code |
| PostgreSQL persistence | Implemented behind `--features postgres`; not default build |
| Password-based account auth | Username format validation only |

---

## How to Update This Document

When closing a roadmap item:

1. Move the checkbox or phase status in this file
2. Update the "Last reviewed" date
3. If a doc/code inconsistency is resolved, remove it from the table above

When adding new planned work, place it in the appropriate tier and note which phase it belongs to (if any).
