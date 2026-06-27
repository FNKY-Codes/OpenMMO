# OpenMMO Roadmap

**Last reviewed:** 2026-06-27

## Status Summary

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

See [docs/testing.md](testing.md). CI runs `cargo test --workspace`, fmt, clippy, content-validator, region-linter.

## Tier priorities

### Tier A — Gameplay polish

- [x] Spell casting UI (Skills tab → `CastSpell`)
- [x] GE unit tests + index fix
- [x] Speed-hack detection on movement steps
- [ ] Chat channel routing (Local/Global/Clan)
- [ ] Full protocol serde tests

### Tier B — Production

- [x] Password auth (postgres feature)
- [x] Redis session role defined
- [ ] Postgres default in release profile
- [ ] WebSocket integration tests

### Tier C — Content

- [x] Second region YAML (`shattered_coast`)
- [x] BETA skills initialized at login (`state.rs`)
- [x] Specialization picker UI
- [x] Collection log client handler
- [x] Minimap player position
- [x] Fishing rod item + shop stock

### Long-term

- [ ] Engine loads sprite atlases (atlas-packer manifest exists; PNG packing next)
- [ ] WASM browser client (`wasm_main` stub)
- [ ] Plugin sandbox (registry hook; WASM/Lua execution)
- [ ] Visual editor (scaffold with item/NPC/region sections)

## Known gaps

| Item | Notes |
|------|-------|
| Postgres persistence | `--features postgres` required |
| Redis | `--features redis` required for sessions |
| Multi-region travel | Second region exists; transitions not wired |
| GE seller inventory | Matching does not debit seller stock yet |

## How to update

When closing roadmap items, update this file and `docs/testing.md`.
