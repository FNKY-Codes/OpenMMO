# OpenMMO Roadmap

This document tracks planned work, current delivery status, and recommended priorities for OpenMMO. It reflects the phased plan from the v0.1 foundation release, updated against what is actually shipped on `main`.

**Last reviewed:** 2026-06-26

## Status Summary

| Phase | Goal | Status |
|-------|------|--------|
| Phase 0 — Foundation | Workspace, engine, protocol, infra, CI | **Complete** |
| Phase 1 — Core loop MVP | Skills, harvest/refine/combat, inventory/bank/chat | **Partial** — server logic exists; client wiring is thin |
| Phase 2 — Content pipeline | YAML content, validator, tutorial quest | **Partial** — pipeline exists; content depth and tooling gaps remain |
| Phase 3 — Economy & social | GE, trading, friends, ledger, minigames | **Scaffolded** — modules and protocol exist; not playable end-to-end |
| Phase 4 — Hardening | Anti-cheat, mod tools, plugin API, docs | **Partial** — docs done; enforcement and persistence incomplete |

**Overall:** The architecture and server-side skeleton are in place (~30% end-to-end). The highest-leverage work is wiring the client to existing server systems so the core loop is actually playable.

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

## Phase 1 — Core Loop MVP ⚠️

**Goal:** A playable skilling and combat loop — harvest, refine, fight, bank, chat.

### Delivered (server-side)

- 600ms authoritative tick loop
- A* pathfinding and walk intent resolution
- Harvest/refine timers with XP grants
- Melee and spell combat with NPC loot tables
- Inventory (28 slots), bank, ground items
- Local chat broadcast
- Six MVP skills initialized on join: Vitality, Prowess, Fortitude, Harvest, Refinement, Arcana

### Delivered (client-side)

- Login flow and WebSocket connection
- 3D world rendering with orbit camera (post-foundation)
- Click-to-walk via ground-plane raycast
- egui panels: inventory (drop only), bank (display), skills, chat, minimap placeholder

### Gaps

- Client only wires **walk** and **chat** — no click-to-harvest, attack, pickup, refine, or bank deposit/withdraw
- Server broadcasts `WorldSnapshot` to all clients; `StateDelta` exists in protocol but is unused
- Equipment stats (`prowess_bonus`, `equip_slot`) are never applied in combat
- Tool requirements (`tool_tag`: axe, pick, rod) are documented but not enforced server-side
- NPC `aggro_range` fields exist but NPCs do not pursue players
- Spells always hit (no accuracy roll)

### Success criteria (remaining)

- [ ] Player can harvest, refine, fight, and bank through normal client interactions
- [ ] XP, damage, and skill updates appear in the HUD without manual protocol messages
- [ ] Equipment affects combat stats
- [ ] Tool tags are enforced for harvest nodes

---

## Phase 2 — Content Pipeline ⚠️

**Goal:** Data-driven game content with validation and a tutorial onboarding path.

### Delivered

- YAML content in `content/` (items, NPCs, objects, regions, quests, dialogues, skills, specializations, recipes, spells, shops)
- `content-validator` tool with orphan-ID checks for loot, recipes, and quests
- Tutorial quest *First Steps in the Reach* and guide dialogues
- Single starter region: `verdant_reach.yaml` (30×30)
- Colored geometry rendering (MVP asset approach)

### Gaps

- `tools/atlas-packer` is referenced in README and `docs/engine.md` but **does not exist**
- Only **3 of 14 skills** have YAML definitions (README claims 14)
- Quest content bug: stage 1 says "Speak with the guide" but `npc_id: 1` is Meadow Crawler (combat mob), not a guide NPC
- Quest objectives with `count` fields are ignored — one action advances a stage regardless
- Objective types `TalkToNpc`, `ReachSkillLevel`, and `VisitTile` are not handled server-side
- Quest journal is built on login but never sent to the client
- Content hot-reload is not implemented (server restart required)
- Validator does not check dialogues, shops, regions, or spell references

### Success criteria (remaining)

- [ ] Tutorial quest is completable as authored
- [ ] All 14 skills have content definitions
- [ ] Content validator covers dialogues, shops, and regions
- [ ] Quest journal is sent to client on login and updates during play
- [ ] `atlas-packer` tool exists (or README/docs updated to reflect geometry MVP)

---

## Phase 3 — Economy & Social ❌

**Goal:** Player-to-player economy, social features, and instanced content.

### Scaffolded (not playable)

| Feature | Protocol | Server logic | Client UI | Settlement |
|---------|----------|--------------|-----------|------------|
| Grand Exchange | Yes | Partial matching | Panel exists, unwired | No item/currency transfer |
| Player trading | Yes | Session tracking | Unwired | `execute_trade()` removes session but swaps nothing |
| Friends / PMs | Yes | One-way friend add | Panel exists, unwired | PMs broadcast globally |
| Ledger contracts | Yes | Kill progress tracking | Unwired | Broadcast every tick to all clients |
| Arena minigame | Yes | Boss spawn + phases | Unwired | Boss not in entity snapshots; no way to damage it |
| Shops | Content only | No handler | No UI | `OpenShop` dialogue action unimplemented |

### Success criteria (remaining)

- [ ] GE offers match and transfer items/currency to players
- [ ] Player trade completes with item swap on both sides
- [ ] PMs route to the intended recipient
- [ ] Shop buying works from `content/shops/`
- [ ] Arena minigame is joinable and the boss is damageable/rendered
- [ ] Ledger updates are per-player, not global broadcast

---

## Phase 4 — Hardening ⚠️

**Goal:** Production readiness for self-hosted shards.

### Delivered

- Architecture, engine, self-hosting, and content-authoring docs
- Moderator commands: kick, ban, teleport, spawn item
- Chat length validation
- In-memory audit log (1000 entries)
- Plugin API hook point (`PluginApi::on_tick`)

### Gaps

- `detect_speed_hack()` is defined but never called
- Moderator commands have no permission check (docs say "requires moderator flag")
- `PluginApi::on_tick` is an empty stub — WASM/Lua sandbox is future work
- PostgreSQL schema exists but no character save/load; default build skips `--features postgres`
- Redis is in Docker Compose but unused in Rust code
- Audit log is not persisted to the database

### Success criteria (remaining)

- [ ] Characters persist across disconnect/reconnect via PostgreSQL
- [ ] Account auth validates username (currently ignored on login)
- [ ] Speed-hack detection runs on movement
- [ ] Moderator commands require a permission flag
- [ ] Audit log persists to DB in production deployments

---

## Post-Foundation Work

These items were delivered after the original Phase 0–4 plan:

| Change | Status | PR |
|--------|--------|-----|
| 3D world rendering (colored blocks on XZ plane) | Done | #3 |
| Orbit camera (right-drag rotate, scroll zoom) | Done | #3 |
| Camera locked on local player while moving | Done | #4, #5 |

---

## Recommended Priority Stack

Work is ordered by dependency and player impact. Complete each tier before expanding scope into the next.

### Tier 1 — Make it playable

Unblocks all downstream work. Nothing else matters until the core loop works through the client.

1. Wire client interactions: harvest, attack, pickup, refine, bank deposit/withdraw
2. Handle server→client messages for XP, damage, quest progress, and dialogue
3. Fix quest system: objective types, progress counters, send journal on login
4. Fix quest content bug (guide NPC vs Meadow Crawler on `npc_id: 1`)
5. Per-player WebSocket routing instead of global broadcast

### Tier 2 — Complete the vertical slice

5. Trade and GE item/currency settlement
6. Equipment equipping and combat stat application
7. Shop buying from `content/shops/`
8. NPC aggro and pursuit
9. Expand content-validator (dialogues, regions, shops, spell refs)
10. Add content beyond the single starter region

### Tier 3 — Persistence and ops

11. Character save/load via PostgreSQL (enable `postgres` feature by default)
12. Account authentication
13. Wire up anti-cheat (`detect_speed_hack`, mod permission checks)
14. Persist audit log to DB
15. Define and implement Redis usage (sessions, cache, or pub/sub)

### Tier 4 — Future (documented elsewhere)

16. `tools/atlas-packer` and sprite rendering (see `docs/engine.md`)
17. Content hot-reload (see `docs/content-authoring.md`)
18. WASM browser client target (see `docs/engine.md`)
19. WASM/Lua plugin sandbox (see `docs/self-hosting.md`, `crates/server/src/anticheat.rs`)

---

## Known Doc/Code Inconsistencies

Track these alongside feature work so contributors are not misled:

| Claim | Reality |
|-------|---------|
| README lists `atlas-packer` under `tools/` | Directory does not exist |
| `docs/architecture.md`: `StateDelta → Client render` | Server only sends `WorldSnapshot` |
| README: "14 consolidated skills" | Only 3 have content YAML; 6 initialized on join |
| Redis in prerequisites and Docker Compose | Zero references in Rust code |
| PostgreSQL persistence | Schema and optional migrations only |
| Self-hosting: moderator flag required | No auth gate on mod commands |

---

## How to Update This Document

When closing a roadmap item:

1. Move the checkbox or phase status in this file
2. Update the "Last reviewed" date
3. If a doc/code inconsistency is resolved, remove it from the table above

When adding new planned work, place it in the appropriate tier and note which phase it belongs to (if any).
