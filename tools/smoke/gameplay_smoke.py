"""Gameplay smoke test: stations, bank, crafting gate, eating, shop selling.

usage: python gameplay_smoke.py ws://127.0.0.1:8080/ws
"""
import asyncio
import json
import sys
import time

import websockets

URL = sys.argv[1] if len(sys.argv) > 1 else "ws://127.0.0.1:8080/ws"
SUFFIX = str(int(time.time()))[-6:]
USER, CHAR, PW = f"gp{SUFFIX}", f"Gp{SUFFIX}", "x"
BANK_CHEST, WORKBENCH = 10, 13  # object ids

results = []


def check(name, ok, detail=""):
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name} {detail}")


class Client:
    def __init__(self, ws):
        self.ws = ws
        self.entities = {}
        self.inventory = None
        self.hp = None
        self.log = []

    async def send(self, msg):
        await self.ws.send(json.dumps(msg))

    def absorb(self, m):
        t = m["type"]
        if t in ("world_snapshot", "region_changed"):
            self.entities = {e["entity_id"]: e for e in m["entities"]}
        elif t == "state_delta":
            for e in m["entities"]:
                self.entities[e["entity_id"]] = e
        elif t == "inventory_update":
            self.inventory = m["inventory"]["slots"]
        elif t == "player_update":
            self.hp = m["hp"]
        self.log.append(m)

    async def wait_for(self, pred, timeout=15):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                raw = await asyncio.wait_for(self.ws.recv(), timeout=max(0.05, deadline - time.time()))
            except asyncio.TimeoutError:
                return None
            m = json.loads(raw)
            self.absorb(m)
            if pred(m):
                return m
        return None

    def find_object(self, object_id):
        for e in self.entities.values():
            k = e["kind"]
            if k.get("type") == "object" and k["object_id"] == object_id:
                return e["entity_id"], k["position"]
        return None, None

    def count(self, item_id):
        return sum(s["quantity"] for s in (self.inventory or []) if s and s["item_id"] == item_id)


async def main():
    print(f"target {URL}; char {CHAR}")
    async with websockets.connect(URL) as ws:
        c = Client(ws)
        await c.send({"type": "login", "username": USER, "character_name": CHAR, "password": PW})
        res = await c.wait_for(lambda m: m["type"] == "login_result")
        check("login", res and res["success"], res and res["message"])
        await c.wait_for(lambda m: m["type"] == "inventory_update", timeout=3)
        check("starter kit: club, axe, pick, 5 minnow, 30 scrap",
              c.count(31) == 1 and c.count(9) == 1 and c.count(10) == 1 and c.count(7) == 5 and c.count(1) == 30,
              f"inv={[s for s in c.inventory if s]}")

        # --- Bank chest: interact -> walk -> StationOpen(bank) ---------------
        eid, pos = c.find_object(BANK_CHEST)
        check("bank chest visible in snapshot", eid is not None, f"at {pos}")
        await c.send({"type": "interact_object", "object_entity": eid})
        st = await c.wait_for(lambda m: m["type"] == "station_open", timeout=20)
        check("walking to the chest opens the bank", st is not None and st["station"] == "bank", st)

        # Deposit 5 scrap from its slot.
        scrap_slot = next(i for i, s in enumerate(c.inventory) if s and s["item_id"] == 1)
        await c.send({"type": "bank_deposit", "inv_slot": scrap_slot, "quantity": 5})
        await c.wait_for(lambda m: m["type"] == "inventory_update", timeout=5)
        check("deposit 5 scrap at the chest", c.count(1) == 25, f"scrap now {c.count(1)}")

        # --- Crafting gate: refine away from any workbench is refused --------
        await c.send({"type": "refine", "recipe_id": "timber_to_plank"})
        err = await c.wait_for(lambda m: m["type"] == "error", timeout=5)
        check("refine away from workbench refused", err is not None and "workbench" in err["message"].lower(), err)

        # --- Walk to a workbench and try again (no timber -> ingredients error)
        eid, pos = c.find_object(WORKBENCH)
        await c.send({"type": "interact_object", "object_entity": eid})
        st = await c.wait_for(lambda m: m["type"] == "station_open", timeout=25)
        check("walking to the workbench opens it", st is not None and st["station"] == "workbench", st)
        await c.send({"type": "refine", "recipe_id": "timber_to_plank"})
        err = await c.wait_for(lambda m: m["type"] == "error", timeout=5)
        check("refine at workbench without timber -> ingredients error",
              err is not None and "ingredient" in err["message"].lower(), err)

        # --- Eating at full HP, then after damage ----------------------------
        minnow_slot = next(i for i, s in enumerate(c.inventory) if s and s["item_id"] == 7)
        await c.send({"type": "use_item", "inv_slot": minnow_slot})
        n = await c.wait_for(lambda m: m["type"] == "notice", timeout=5)
        check("eating at full HP is a no-op notice", n is not None and "not hurt" in n["message"].lower(), n)

        # --- Shop selling --------------------------------------------------
        axe_slot = next(i for i, s in enumerate(c.inventory) if s and s["item_id"] == 9)
        before = c.count(1)
        await c.send({"type": "shop_sell", "shop_id": "general_store", "inv_slot": axe_slot, "quantity": 1})
        n = await c.wait_for(lambda m: m["type"] == "notice", timeout=5)
        check("sell training axe for 20 scrap", c.count(9) == 0 and c.count(1) == before + 20, f"{n} scrap={c.count(1)}")

        # --- Equip level gate ---------------------------------------------
        club_slot = next(i for i, s in enumerate(c.inventory) if s and s["item_id"] == 31)
        await c.send({"type": "equip_item", "inv_slot": club_slot})
        inv = await c.wait_for(lambda m: m["type"] == "inventory_update", timeout=5)
        check("equip scrap club (level 1)", inv is not None and inv["equipment"]["weapon"] is not None, inv and inv.get("equipment"))

    failed = [n for n, ok in results if not ok]
    print(f"\n{len(results) - len(failed)}/{len(results)} passed")
    sys.exit(1 if failed else 0)


asyncio.run(main())
