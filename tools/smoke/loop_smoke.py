"""Core loop smoke test: chop timber -> saw a plank -> fight a rat -> loot.

usage: python loop_smoke.py ws://127.0.0.1:8080/ws
"""
import asyncio
import json
import sys
import time

import websockets

URL = sys.argv[1] if len(sys.argv) > 1 else "ws://127.0.0.1:8080/ws"
SUFFIX = str(int(time.time()))[-6:]
USER, CHAR, PW = f"lp{SUFFIX}", f"Lp{SUFFIX}", "x"
BIRCH, WORKBENCH = 1, 13
RAT = 4
TIMBER, PLANK, SCRAP = 2, 3, 1

results = []


def check(name, ok, detail=""):
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name} {detail}")


class Client:
    def __init__(self, ws):
        self.ws = ws
        self.entities = {}
        self.inventory = None
        self.me = None
        self.pos = None
        self.log = []

    async def send(self, msg):
        await self.ws.send(json.dumps(msg))

    def absorb(self, m):
        t = m["type"]
        if t == "login_result":
            self.me = m.get("player_id")
        if t in ("world_snapshot", "region_changed"):
            self.entities = {e["entity_id"]: e for e in m["entities"]}
        elif t == "state_delta":
            for e in m["entities"]:
                self.entities[e["entity_id"]] = e
        elif t == "inventory_update":
            self.inventory = m["inventory"]["slots"]
        elif t == "player_update" and m["player_id"] == self.me:
            self.pos = m["position"]
        elif t == "death":
            self.entities.pop(m["entity"], None)
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

    def my_pos(self):
        for e in self.entities.values():
            k = e["kind"]
            if k.get("type") == "player" and k["player_id"] == self.me:
                return k["position"]
        return self.pos

    def nearest(self, pred):
        me = self.my_pos() or {"x": 40, "y": 44}
        best = None
        for e in self.entities.values():
            k = e["kind"]
            if not pred(k):
                continue
            p = k["position"]
            d = max(abs(p["x"] - me["x"]), abs(p["y"] - me["y"]))
            if best is None or d < best[0]:
                best = (d, e["entity_id"], p)
        return best

    def count(self, item_id):
        return sum(s["quantity"] for s in (self.inventory or []) if s and s["item_id"] == item_id)


async def main():
    print(f"target {URL}; char {CHAR}")
    async with websockets.connect(URL) as ws:
        c = Client(ws)
        await c.send({"type": "login", "username": USER, "character_name": CHAR, "password": PW})
        res = await c.wait_for(lambda m: m["type"] == "login_result")
        check("login", res and res["success"])
        await c.wait_for(lambda m: m["type"] == "inventory_update", timeout=3)

        # --- Skilling: chop the nearest birch ---------------------------------
        tree = c.nearest(lambda k: k.get("type") == "object" and k["object_id"] == BIRCH and not k.get("depleted"))
        check("found a birch", tree is not None, f"{tree}")
        await c.send({"type": "scavenge", "object_entity": tree[1]})
        xp = await c.wait_for(lambda m: m["type"] == "xp_drop" and m["skill"] == "scavenging", timeout=40)
        check("chopping grants Scavenging xp", xp is not None, xp)
        # inventory_update precedes xp_drop within the tick, so it's already absorbed.
        check("timber in inventory", c.count(TIMBER) >= 1, f"timber={c.count(TIMBER)}")
        # The node stays visible and is flagged depleted in the next delta,
        # then respawns (birch: 12 ticks) without disappearing in between.
        tid = tree[1]
        seen = []
        def dep_true(m):
            if m["type"] != "state_delta":
                return False
            for e in m["entities"]:
                if e["entity_id"] == tid:
                    seen.append(e["kind"].get("depleted"))
                    return e["kind"].get("depleted") is True
            return False
        d = await c.wait_for(dep_true, timeout=5)
        check("node is marked depleted, not removed", d is not None, f"seen={seen[:6]}")
        r = await c.wait_for(lambda m: m["type"] == "state_delta" and any(
            e["entity_id"] == tid and e["kind"].get("depleted") is False for e in m["entities"]), timeout=20)
        check("node respawns", r is not None)

        # --- Crafting: saw it into a plank at the workbench ---------------------
        bench = c.nearest(lambda k: k.get("type") == "object" and k["object_id"] == WORKBENCH)
        await c.send({"type": "interact_object", "object_entity": bench[1]})
        st = await c.wait_for(lambda m: m["type"] == "station_open", timeout=40)
        check("reached the workbench", st is not None and st["station"] == "workbench", st)
        await c.send({"type": "refine", "recipe_id": "timber_to_plank"})
        xp = await c.wait_for(lambda m: m["type"] == "xp_drop" and m["skill"] == "fabrication", timeout=20)
        check("sawing grants Fabrication xp", xp is not None, xp)
        check("plank made", c.count(PLANK) >= 1, f"plank={c.count(PLANK)}")

        # --- Combat: kill a Mutant Rat and take the drop ------------------------
        rat = c.nearest(lambda k: k.get("type") == "npc" and k["npc_id"] == RAT and k["hp"] > 0)
        check("found a rat", rat is not None, f"{rat}")
        # wield the club first
        club = next(i for i, s in enumerate(c.inventory) if s and s["item_id"] == 31)
        await c.send({"type": "equip_item", "inv_slot": club})
        await c.wait_for(lambda m: m["type"] == "inventory_update", timeout=5)
        await c.send({"type": "attack", "target": rat[1], "style": "melee"})
        hit = await c.wait_for(lambda m: m["type"] == "damage" and m["target"] == rat[1], timeout=60)
        check("we land a hit on the rat", hit is not None, hit)
        death = await c.wait_for(lambda m: m["type"] == "death" and m["entity"] == rat[1], timeout=90)
        check("rat dies", death is not None, death)
        cxp = [m for m in c.log if m["type"] == "xp_drop" and m["skill"] == "combat"]
        check("combat xp was granted", len(cxp) > 0, cxp[:1])
        loot = await c.wait_for(lambda m: m["type"] == "loot_spawn", timeout=5)
        if loot and loot["items"]:
            before = c.count(SCRAP)
            g = loot["items"][0]
            await c.send({"type": "pickup_item", "ground_entity": g["entity_id"]})
            await c.wait_for(lambda m: m["type"] == "inventory_update", timeout=15)
            check("picked up the drop", c.count(SCRAP) > before or c.count(g["item_id"]) > 0, f"scrap {before}->{c.count(SCRAP)}")
        else:
            check("no drop this kill (10% chance) - skipped pickup", True)

    failed = [n for n, ok in results if not ok]
    print(f"\n{len(results) - len(failed)}/{len(results)} passed")
    sys.exit(1 if failed else 0)


asyncio.run(main())
