"""End-to-end login/persistence smoke test against a running OpenMMO server.

usage: python smoke.py wss://host/ws
"""
import asyncio
import json
import sys
import time

import websockets

URL = sys.argv[1] if len(sys.argv) > 1 else "ws://127.0.0.1:8080/ws"
SUFFIX = str(int(time.time()))[-6:]
USER_A, USER_B = f"smokeA{SUFFIX}", f"smokeB{SUFFIX}"
CHAR_A = f"SmokeChar{SUFFIX}"
PW = "correct-horse"

results = []


def check(name, ok, detail=""):
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name} {detail}")


async def recv_until(ws, types, timeout=15):
    """Return the first message whose type is in `types`, collecting inventory on the way."""
    seen = {}
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            raw = await asyncio.wait_for(ws.recv(), timeout=max(0.1, deadline - time.time()))
        except asyncio.TimeoutError:
            break
        except websockets.exceptions.ConnectionClosed as e:
            print(f"    (socket closed: {e})")
            break
        msg = json.loads(raw)
        seen[msg["type"]] = msg
        if msg["type"] in types:
            return msg, seen
    return None, seen


async def login(ws, user, char, pw):
    await ws.send(json.dumps({"type": "login", "username": user, "character_name": char, "password": pw}))
    return await recv_until(ws, {"login_result"})


def slot0_qty(seen):
    inv = seen.get("inventory_update", {}).get("inventory", {})
    slots = inv.get("slots", [])
    s0 = slots[0] if slots else None
    return (s0 or {}).get("quantity")


async def main():
    print(f"target {URL}; users {USER_A}/{USER_B}; char {CHAR_A}")

    # 1. First login creates account + character.
    async with websockets.connect(URL) as ws:
        res, seen = await login(ws, USER_A, CHAR_A, PW)
        check("first login succeeds", bool(res and res["success"]), res and res["message"])
        q_before = slot0_qty(seen)
        check("starter inventory present", q_before is not None, f"slot0 qty={q_before}")
        # Mutate state: drop 1 of slot 0, then wait for the inventory echo.
        await ws.send(json.dumps({"type": "drop_item", "slot": 0, "quantity": 1}))
        _, seen2 = await recv_until(ws, {"inventory_update"}, timeout=5)
        q_after_drop = slot0_qty(seen2)
        check("drop changes inventory", q_after_drop != q_before, f"{q_before} -> {q_after_drop}")

    await asyncio.sleep(1)  # let the disconnect save land

    # 2. Wrong password is refused.
    async with websockets.connect(URL) as ws:
        res, _ = await login(ws, USER_A, CHAR_A, "wrong")
        check("wrong password refused", bool(res and not res["success"]), res and res["message"])

    # 3. Another account cannot take the character.
    async with websockets.connect(URL) as ws:
        res, _ = await login(ws, USER_B, CHAR_A, PW)
        check("other account cannot use character", bool(res and not res["success"]), res and res["message"])

    # 4. Relogin restores the mutated inventory (persistence).
    async with websockets.connect(URL) as ws_first:
        res, seen = await login(ws_first, USER_A, CHAR_A, PW)
        check("relogin succeeds", bool(res and res["success"]), res and res["message"])
        q_reloaded = slot0_qty(seen)
        check("inventory persisted across sessions", q_reloaded == q_after_drop, f"reloaded={q_reloaded} expected={q_after_drop}")

        # 5. Duplicate login evicts the first session.
        async with websockets.connect(URL) as ws_second:
            res2, _ = await login(ws_second, USER_A, CHAR_A, PW)
            check("second session logs in", bool(res2 and res2["success"]), res2 and res2["message"])
            evicted, _ = await recv_until(ws_first, {"error"}, timeout=5)
            check("first session receives eviction error", evicted is not None, evicted and evicted.get("message"))
            try:
                await asyncio.wait_for(ws_first.wait_closed(), timeout=5)
                closed = True
            except asyncio.TimeoutError:
                closed = False
            check("first session socket closed", closed)

    failed = [n for n, ok in results if not ok]
    print(f"\n{len(results) - len(failed)}/{len(results)} passed")
    sys.exit(1 if failed else 0)


asyncio.run(main())
