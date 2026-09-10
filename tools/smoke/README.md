# Smoke tests

Scripted WebSocket clients that exercise the live server. They need
`python -m pip install websockets`.

```
python tools/smoke/login_smoke.py     wss://server-production-2a39.up.railway.app/ws
python tools/smoke/gameplay_smoke.py  ws://127.0.0.1:8080/ws
```

- `login_smoke.py` — accounts, passwords, character ownership, persistence
  across sessions, duplicate-login eviction.
- `gameplay_smoke.py` — starter kit, walking to a bank chest / workbench
  opens the station, bank deposit, crafting station + ingredient gates,
  eating, shop selling, equipping.
- `loop_smoke.py` — the core loop: chop a birch (XP, timber, node depletes
  then respawns), saw a plank at the workbench, wield the club, kill a
  Mutant Rat (hits, death, Combat XP) and pick up the drop.

These do **not** exercise the Rust client's own network stack; for that run
`cargo test -p openmmo-client --test wss_login -- --ignored`.
