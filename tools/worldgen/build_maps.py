#!/usr/bin/env python3
"""Build the region YAML files under content/regions from hand-placed districts.

Regions are composed from primitives (rects, roads, buildings, seeded scatter)
so the layout is reproducible, then written as ASCII `layout` rows plus a
`legend`, which the engine expands on load (see crates/sdk/src/tiles.rs).

    python tools/worldgen/build_maps.py          # writes content/regions/*.yaml

The emitted YAML is plain text art and can be hand-edited afterwards; re-run
this script only if you want to regenerate from the source of truth here.
"""
from __future__ import annotations

import random
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "content" / "regions"

# --- tile kinds (must match TileKind in crates/sdk/src/tiles.rs) -----------
GRASS, ROCK, WATER, DIRT, ROAD, SAND, RUBBLE, CAVE, CLIFF, WALL, FLOOR, MUD, ASH, SNOW, SHALLOWS, HAZARD = (
    "grass", "rock", "water", "dirt", "road", "sand", "rubble", "cave_floor", "cliff",
    "wall", "floor", "mud", "ash", "snow", "shallows", "hazard",
)
BLOCKED = {ROCK, WATER, CLIFF, WALL, HAZARD}

# --- object ids (content/objects) -------------------------------------------
BIRCH, COPPER, POOL, OAK, IRONWOOD, IRON_VEIN, COAL, RIVER, DEEP_POOL = 1, 2, 3, 4, 5, 6, 7, 8, 9
BANK, FURNACE, ANVIL, WORKBENCH, COOKFIRE, TRADE_BOARD = 10, 11, 12, 13, 14, 15
LAMP, CRATE, BARREL, FENCE, DEAD_TREE, BOULDER, SIGN, RUBBLE_PILE, MUSHROOM = 16, 17, 18, 19, 20, 21, 22, 23, 24
STATUE, WELL, TENT, CRYSTAL, STUMP, CAMPFIRE = 25, 26, 27, 28, 29, 30

# --- npc ids (content/npcs) -------------------------------------------------
TIGER, BANDIT, WISP, RAT, HOUND, LURKER, MARKSMAN, BRUTE, MATRON, WARLORD, STALKER = 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11
GUIDE, MARLA, GRIST, WEN = 100, 101, 102, 103

# --- region ids -------------------------------------------------------------
VERDANT, COAST, HOLLOW, WARREN, RIDGE = 1, 2, 3, 4, 5


@dataclass(frozen=True)
class Cell:
    tile: str
    obj: int | None = None
    npc: int | None = None
    spawn: bool = False
    portal: tuple | None = None  # (region, x, y, kind, label)


@dataclass
class Region:
    id: int
    name: str
    w: int
    h: int
    base: str
    ambience: str = "overworld"
    recommended_level: int = 0
    cells: list[list[Cell]] = field(init=False)
    rng: random.Random = field(init=False)

    def __post_init__(self):
        self.cells = [[Cell(self.base) for _ in range(self.w)] for _ in range(self.h)]
        self.rng = random.Random(self.id * 7919)

    # -- primitives ---------------------------------------------------------
    def inb(self, x, y):
        return 0 <= x < self.w and 0 <= y < self.h

    def tile(self, x, y):
        return self.cells[y][x].tile

    def set_tile(self, x, y, tile):
        if self.inb(x, y):
            c = self.cells[y][x]
            self.cells[y][x] = Cell(tile, c.obj, c.npc, c.spawn, c.portal)

    def fill(self, x0, y0, x1, y1, tile):
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                self.set_tile(x, y, tile)

    def border(self, tile, thickness=1):
        for t in range(thickness):
            self.fill(t, t, self.w - 1 - t, t, tile)
            self.fill(t, self.h - 1 - t, self.w - 1 - t, self.h - 1 - t, tile)
            self.fill(t, t, t, self.h - 1 - t, tile)
            self.fill(self.w - 1 - t, t, self.w - 1 - t, self.h - 1 - t, tile)

    def hline(self, x0, x1, y, tile, width=1):
        for dy in range(width):
            self.fill(min(x0, x1), y + dy, max(x0, x1), y + dy, tile)

    def vline(self, x, y0, y1, tile, width=1):
        for dx in range(width):
            self.fill(x + dx, min(y0, y1), x + dx, max(y0, y1), tile)

    def blob(self, cx, cy, r, tile, jitter=0.35):
        """Roughly circular patch."""
        for y in range(cy - r - 1, cy + r + 2):
            for x in range(cx - r - 1, cx + r + 2):
                if not self.inb(x, y):
                    continue
                d = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5
                if d <= r + self.rng.uniform(-jitter, jitter) * r:
                    self.set_tile(x, y, tile)

    def put(self, x, y, obj=None, npc=None, spawn=False, portal=None, tile=None):
        assert self.inb(x, y), f"{self.name}: ({x},{y}) out of bounds"
        c = self.cells[y][x]
        t = tile or c.tile
        if npc is not None or spawn or portal is not None:
            assert t not in BLOCKED, f"{self.name}: placing on blocked tile at ({x},{y})"
        self.cells[y][x] = Cell(t, obj if obj is not None else c.obj, npc if npc is not None else c.npc,
                                spawn or c.spawn, portal or c.portal)

    def building(self, x0, y0, x1, y1, door: tuple[int, int], floor=FLOOR):
        """Walls with an interior floor; `door` is a wall cell replaced by floor."""
        self.fill(x0, y0, x1, y1, WALL)
        self.fill(x0 + 1, y0 + 1, x1 - 1, y1 - 1, floor)
        dx, dy = door
        assert self.tile(dx, dy) == WALL, f"{self.name}: door not on a wall ({dx},{dy})"
        self.set_tile(dx, dy, floor)

    def prune_unreachable(self, spawn: tuple[int, int], fill: str):
        """Turn every walkable tile not connected to `spawn` (8-dir) into
        `fill`, so carvers can't leave isolated pockets that nodes land in."""
        sx, sy = spawn
        seen = {(sx, sy)}
        stack = [(sx, sy)]
        while stack:
            x, y = stack.pop()
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    n = (x + dx, y + dy)
                    if n in seen or not self.inb(*n) or self.tile(*n) in BLOCKED:
                        continue
                    seen.add(n)
                    stack.append(n)
        pruned = 0
        for y in range(self.h):
            for x in range(self.w):
                if self.tile(x, y) not in BLOCKED and (x, y) not in seen:
                    self.set_tile(x, y, fill)
                    pruned += 1
        return pruned

    def free(self, x, y, tiles=None):
        if not self.inb(x, y):
            return False
        c = self.cells[y][x]
        if c.obj is not None or c.npc is not None or c.spawn or c.portal is not None:
            return False
        if tiles is not None and c.tile not in tiles:
            return False
        return c.tile not in BLOCKED

    def scatter(self, x0, y0, x1, y1, density, obj=None, npc=None, tiles=None, min_gap=1):
        """Seeded scatter; never on blocked/occupied tiles, keeps a gap so
        nodes don't wall off paths."""
        placed = []
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                if self.rng.random() >= density or not self.free(x, y, tiles):
                    continue
                if any(abs(px - x) <= min_gap and abs(py - y) <= min_gap for px, py in placed):
                    continue
                self.put(x, y, obj=obj, npc=npc)
                placed.append((x, y))
        return placed

    # -- emit ---------------------------------------------------------------
    def emit(self) -> str:
        combos: dict[Cell, str] = {}
        preferred = {
            GRASS: ".", DIRT: ",", ROAD: "=", SAND: ":", WATER: "~", SHALLOWS: "-", ROCK: "#",
            CLIFF: "^", WALL: "|", FLOOR: "_", RUBBLE: "%", CAVE: "'", MUD: ";", ASH: "`",
            SNOW: "*", HAZARD: "!",
        }
        pool = list("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789@$&+?<>()[]{}/\\")
        used = set()

        def char_for(c: Cell) -> str:
            if c in combos:
                return combos[c]
            plain = c.obj is None and c.npc is None and not c.spawn and c.portal is None
            ch = None
            if plain and preferred[c.tile] not in used:
                ch = preferred[c.tile]
            elif c.spawn and "S" not in used:
                ch = "S"
            elif c.portal is not None and "P" not in used:
                ch = "P"
            if ch is None:
                ch = next(p for p in pool if p not in used)
            used.add(ch)
            combos[c] = ch
            return ch

        rows = ["".join(char_for(c) for c in row) for row in self.cells]
        spawn = [(x, y) for y, row in enumerate(self.cells) for x, c in enumerate(row) if c.spawn]
        assert len(spawn) == 1, f"{self.name}: need exactly one spawn, got {spawn}"

        out = [
            f"# {self.name} — generated by tools/worldgen/build_maps.py; hand-edits are fine.",
            f"id: {self.id}",
            f"name: {self.name}",
            f"ambience: {self.ambience}",
            f"recommended_level: {self.recommended_level}",
            "layout:",
        ]
        out += [f'  - "{r}"' for r in rows]
        out.append("legend:")
        for cell, ch in sorted(combos.items(), key=lambda kv: kv[1]):
            parts = [f"tile: {cell.tile}"]
            if cell.obj is not None:
                parts.append(f"object: {cell.obj}")
            if cell.npc is not None:
                parts.append(f"npc: {cell.npc}")
            if cell.spawn:
                parts.append("spawn: true")
            if cell.portal is not None:
                r, px, py, kind, label = cell.portal
                parts.append(f'portal: {{ region: {r}, spawn: [{px}, {py}], kind: {kind}, label: "{label}" }}')
            key = f'"{ch}"' if ch not in "\\" else '"\\\\"'
            if ch == '"':
                key = "'\"'"
            out.append(f"  {key}: {{ {', '.join(parts)} }}")
        return "\n".join(out) + "\n"


# ============================================================================
# Region 1 — Verdant Reach (overworld hub, 80x80)
# ============================================================================
def verdant_reach() -> Region:
    r = Region(VERDANT, "Verdant Reach", 80, 80, GRASS, "overworld", 1)
    r.border(CLIFF, 2)

    # Main roads meet at the town square.
    r.hline(2, 77, 40, ROAD, 2)          # east-west
    r.vline(40, 2, 77, ROAD, 2)          # north-south

    # --- Town square (centre) --------------------------------------------
    r.fill(30, 30, 51, 51, DIRT)
    r.fill(33, 33, 48, 48, ROAD)
    # well + guide + statue + lamps
    r.put(41, 41, obj=WELL)
    r.put(43, 42, npc=GUIDE)
    r.put(38, 38, obj=STATUE)
    for (x, y) in [(33, 33), (48, 33), (33, 48), (48, 48), (36, 41), (45, 41)]:
        r.put(x, y, obj=LAMP)
    r.put(44, 38, obj=SIGN)
    r.put(40, 44, spawn=True)

    # Marla's general store (NW of square)
    r.building(30, 29, 37, 34, door=(34, 34))
    r.put(33, 31, npc=MARLA)
    r.put(31, 30, obj=CRATE)
    r.put(36, 30, obj=BARREL)
    # Grist's smithy (NE of square): furnace + anvil in the yard
    r.building(44, 29, 51, 34, door=(47, 34))
    r.put(47, 31, npc=GRIST)
    r.put(50, 30, obj=CRATE)
    r.put(44, 36, obj=FURNACE, tile=ROAD)
    r.put(46, 36, obj=FURNACE, tile=ROAD)
    r.put(49, 36, obj=ANVIL, tile=ROAD)
    r.put(51, 36, obj=ANVIL, tile=ROAD)
    # Bank house (S of square), two chests
    r.building(36, 46, 45, 51, door=(40, 46))
    r.put(38, 49, obj=BANK)
    r.put(43, 49, obj=BANK)
    # Workbenches + cookfires + trade board around the square
    r.put(31, 37, obj=WORKBENCH, tile=DIRT)
    r.put(31, 39, obj=WORKBENCH, tile=DIRT)
    r.put(31, 43, obj=COOKFIRE, tile=DIRT)
    r.put(31, 45, obj=COOKFIRE, tile=DIRT)
    r.put(50, 44, obj=TRADE_BOARD, tile=DIRT)
    r.put(50, 46, obj=TRADE_BOARD, tile=DIRT)
    for x in range(30, 52, 3):
        if r.free(x, 52):
            r.put(x, 52, obj=FENCE)

    # --- Birch Woods (west) -----------------------------------------------
    r.fill(3, 18, 27, 62, GRASS)
    r.hline(3, 29, 40, ROAD, 2)  # road continues west
    r.scatter(4, 18, 27, 62, 0.14, obj=BIRCH, tiles={GRASS})
    r.scatter(4, 18, 12, 62, 0.08, obj=OAK, tiles={GRASS})
    r.scatter(4, 18, 27, 62, 0.02, obj=STUMP, tiles={GRASS})
    r.scatter(4, 18, 27, 62, 0.02, obj=DEAD_TREE, tiles={GRASS})
    r.scatter(6, 20, 26, 60, 0.012, npc=TIGER, tiles={GRASS}, min_gap=4)

    # --- Outskirts training field (around town) ---------------------------
    r.scatter(24, 22, 58, 28, 0.03, npc=RAT, tiles={GRASS}, min_gap=3)
    r.scatter(24, 54, 58, 58, 0.03, npc=RAT, tiles={GRASS}, min_gap=3)
    r.scatter(53, 30, 58, 50, 0.03, npc=RAT, tiles={GRASS}, min_gap=3)

    # --- Copper Ridge (north) ---------------------------------------------
    for (cx, cy, rad) in [(30, 12, 3), (37, 9, 4), (48, 11, 3), (55, 14, 3), (44, 16, 2), (34, 18, 2)]:
        r.blob(cx, cy, rad, ROCK)
    r.vline(40, 2, 29, ROAD, 2)
    r.hline(28, 60, 20, DIRT)
    # veins hug the rock edges
    def edge_of_rock(x, y):
        if r.tile(x, y) != GRASS and r.tile(x, y) != DIRT:
            return False
        return any(r.inb(x + dx, y + dy) and r.tile(x + dx, y + dy) == ROCK
                   for dx in (-1, 0, 1) for dy in (-1, 0, 1))
    cand = [(x, y) for y in range(5, 24) for x in range(26, 60) if edge_of_rock(x, y) and r.free(x, y)]
    r.rng.shuffle(cand)
    placed = []
    for (x, y) in cand:
        if len(placed) >= 12:
            break
        if any(abs(px - x) <= 1 and abs(py - y) <= 1 for px, py in placed):
            continue
        r.put(x, y, obj=COPPER)
        placed.append((x, y))
    for (x, y) in cand[len(placed):]:
        if len(placed) >= 15:
            break
        if any(abs(px - x) <= 1 and abs(py - y) <= 1 for px, py in placed):
            continue
        r.put(x, y, obj=IRON_VEIN)
        placed.append((x, y))
    r.scatter(26, 6, 60, 22, 0.015, obj=BOULDER, tiles={GRASS, DIRT})
    r.scatter(50, 4, 74, 24, 0.014, npc=HOUND, tiles={GRASS}, min_gap=4)
    # Cave mouth into Copper Hollow, set into the north cliff
    r.set_tile(40, 2, DIRT)
    r.set_tile(41, 2, DIRT)
    r.put(40, 3, portal=(HOLLOW, 18, 27, "cave", "Cave mouth — Copper Hollow (lvl 12+)"), tile=DIRT)
    r.put(39, 3, obj=BOULDER)
    r.put(42, 3, obj=BOULDER)

    # --- Still Pools (south) ----------------------------------------------
    for (cx, cy, rad) in [(34, 66, 4), (47, 70, 5), (40, 60, 2), (55, 62, 3)]:
        r.blob(cx, cy, rad, WATER)
    # shallows ring around water
    for y in range(54, 78):
        for x in range(26, 62):
            if r.tile(x, y) == GRASS and any(
                r.inb(x + dx, y + dy) and r.tile(x + dx, y + dy) == WATER for dx in (-1, 0, 1) for dy in (-1, 0, 1)
            ):
                r.set_tile(x, y, SHALLOWS)
    pools = [(x, y) for y in range(54, 78) for x in range(26, 62) if r.tile(x, y) == SHALLOWS and r.free(x, y)]
    r.rng.shuffle(pools)
    got = []
    for (x, y) in pools:
        if len(got) >= 9:
            break
        if any(abs(px - x) <= 2 and abs(py - y) <= 2 for px, py in got):
            continue
        r.put(x, y, obj=POOL)
        got.append((x, y))
    r.put(41, 57, npc=WEN, tile=DIRT)
    r.put(43, 57, obj=COOKFIRE, tile=DIRT)
    r.scatter(26, 54, 62, 77, 0.03, obj=BIRCH, tiles={GRASS})

    # --- Old Ruins (south-west) -> Wisp Warren ----------------------------
    r.fill(4, 62, 24, 76, RUBBLE)
    for (x0, y0, x1, y1) in [(6, 64, 12, 68), (16, 66, 22, 70), (8, 71, 14, 75)]:
        r.fill(x0, y0, x1, y1, WALL)
        r.fill(x0 + 1, y0 + 1, x1 - 1, y1 - 1, RUBBLE)
        # knock holes in the walls
        for _ in range(4):
            hx = r.rng.randint(x0, x1)
            hy = r.rng.choice([y0, y1])
            r.set_tile(hx, hy, RUBBLE)
            hx = r.rng.choice([x0, x1])
            hy = r.rng.randint(y0, y1)
            r.set_tile(hx, hy, RUBBLE)
    r.scatter(4, 62, 24, 76, 0.05, obj=RUBBLE_PILE, tiles={RUBBLE})
    r.scatter(4, 62, 24, 76, 0.02, obj=DEAD_TREE, tiles={RUBBLE})
    r.scatter(5, 63, 23, 75, 0.02, npc=WISP, tiles={RUBBLE}, min_gap=3)
    r.hline(20, 29, 41, DIRT)          # path from the west road down
    r.vline(20, 41, 61, DIRT)
    r.put(19, 68, portal=(WARREN, 20, 37, "door", "Iron door — Wisp Warren (lvl 18+)"), tile=RUBBLE)
    r.set_tile(18, 68, WALL)
    r.set_tile(20, 68, WALL)

    # --- Bandit Outskirts (east) -> coast road, ridge stairs --------------
    r.fill(58, 26, 76, 56, DIRT)
    r.hline(52, 77, 40, ROAD, 2)
    r.vline(60, 28, 54, FENCE if False else DIRT)
    for y in range(28, 55, 2):
        if r.free(59, y) and r.tile(59, y) != ROAD:
            r.put(59, y, obj=FENCE)
    camps = [(64, 31), (70, 33), (66, 48), (72, 51), (74, 36)]
    for (x, y) in camps:
        r.put(x, y, obj=TENT)
        r.put(x + 1, y + 1, obj=CAMPFIRE)
        r.put(x - 1, y + 1, npc=BANDIT)
    r.scatter(60, 27, 76, 55, 0.01, npc=BANDIT, tiles={DIRT}, min_gap=4)
    r.scatter(60, 27, 76, 55, 0.02, obj=CRATE, tiles={DIRT})
    r.scatter(60, 27, 76, 55, 0.02, obj=BARREL, tiles={DIRT})
    r.scatter(60, 27, 76, 55, 0.03, obj=DEAD_TREE, tiles={DIRT})
    # Coast road exit (east edge)
    r.set_tile(78, 40, ROAD)
    r.set_tile(78, 41, ROAD)
    r.put(78, 40, portal=(COAST, 2, 15, "path", "Coast road — Shattered Coast (lvl 15+)"))
    r.put(78, 41, portal=(COAST, 2, 16, "path", "Coast road — Shattered Coast (lvl 15+)"))
    r.put(77, 38, obj=SIGN)
    # Stairs up the ridge (north-east cliffs)
    r.vline(70, 6, 26, DIRT)
    r.hline(60, 70, 20, DIRT)
    r.set_tile(70, 5, DIRT)
    r.put(70, 5, portal=(RIDGE, 3, 27, "stairs_up", "Stairs up — Bandit Ridge (lvl 25+)"))
    r.put(69, 5, obj=BOULDER)
    r.put(71, 5, obj=BOULDER)
    r.scatter(62, 8, 76, 18, 0.03, obj=BOULDER, tiles={GRASS, DIRT})

    r.prune_unreachable((40, 44), CLIFF)
    return r


# ============================================================================
# Region 2 — Shattered Coast (44x30)
# ============================================================================
def shattered_coast() -> Region:
    r = Region(COAST, "Shattered Coast", 44, 30, GRASS, "coast", 15)
    r.border(CLIFF, 1)
    # sea along the east and south
    for y in range(1, 29):
        for x in range(1, 43):
            d = min(43 - x, 29 - y)
            if d <= 3:
                r.set_tile(x, y, WATER)
            elif d <= 5:
                r.set_tile(x, y, SHALLOWS)
            elif d <= 9:
                r.set_tile(x, y, SAND)
    # river from the north edge down to the sea
    for y in range(1, 24):
        x = 22 + int(3 * ((y % 9) / 9.0 - 0.5) * 2)
        r.fill(x, y, x + 1, y, WATER)
        r.set_tile(x - 1, y, SHALLOWS)
        r.set_tile(x + 2, y, SHALLOWS)
    # road in from the west, bridge over the river (shallows), along the shore
    r.hline(1, 40, 15, ROAD, 2)
    for x in range(19, 27):
        for y in (15, 16):
            if r.tile(x, y) == WATER:
                r.set_tile(x, y, SHALLOWS)
    r.put(2, 15, spawn=True)
    r.put(1, 15, portal=(VERDANT, 76, 40, "path", "Road west — Verdant Reach"), tile=ROAD)
    r.put(1, 16, portal=(VERDANT, 76, 41, "path", "Road west — Verdant Reach"), tile=ROAD)
    r.put(4, 13, obj=SIGN)
    # river shallows fishing spots
    spots = [(x, y) for y in range(2, 24) for x in range(18, 28) if r.tile(x, y) == SHALLOWS and r.free(x, y) and y not in (15, 16)]
    r.rng.shuffle(spots)
    got = []
    for (x, y) in spots:
        if len(got) >= 7:
            break
        if any(abs(px - x) <= 2 and abs(py - y) <= 2 for px, py in got):
            continue
        r.put(x, y, obj=RIVER)
        got.append((x, y))
    # deep pools in the sea shallows, far end
    r.put(36, 24, obj=DEEP_POOL)
    r.put(39, 20, obj=DEEP_POOL)
    r.put(30, 26, obj=DEEP_POOL)
    # oaks inland, dunes with marksmen, hounds near the road
    r.scatter(2, 2, 16, 12, 0.12, obj=OAK, tiles={GRASS})
    r.scatter(2, 18, 16, 27, 0.12, obj=OAK, tiles={GRASS})
    r.scatter(28, 2, 40, 12, 0.02, npc=MARKSMAN, tiles={SAND, GRASS}, min_gap=4)
    r.scatter(28, 18, 38, 26, 0.02, npc=MARKSMAN, tiles={SAND, GRASS}, min_gap=4)
    r.scatter(6, 3, 30, 27, 0.01, npc=HOUND, tiles={GRASS, SAND}, min_gap=5)
    r.scatter(2, 2, 40, 27, 0.02, obj=BOULDER, tiles={SAND, GRASS})
    r.scatter(2, 2, 40, 27, 0.015, obj=DEAD_TREE, tiles={SAND, GRASS})
    r.put(33, 8, obj=TENT)
    r.put(34, 9, obj=CAMPFIRE)
    r.put(34, 12, obj=COOKFIRE, tile=SAND)
    return r


# ============================================================================
# Region 3 — Copper Hollow (cave, 40x30)
# ============================================================================
def copper_hollow() -> Region:
    r = Region(HOLLOW, "Copper Hollow", 40, 30, ROCK, "cave", 12)
    # carve chambers and tunnels
    for (cx, cy, rad) in [(18, 25, 3), (18, 18, 4), (9, 14, 4), (28, 14, 5), (12, 6, 4), (30, 5, 3), (20, 9, 3)]:
        r.blob(cx, cy, rad, CAVE, jitter=0.5)
    for (a, b) in [((18, 25), (18, 18)), ((18, 18), (9, 14)), ((18, 18), (28, 14)), ((9, 14), (12, 6)),
                   ((28, 14), (30, 5)), ((12, 6), (20, 9)), ((20, 9), (30, 5))]:
        (x0, y0), (x1, y1) = a, b
        x, y = x0, y0
        while (x, y) != (x1, y1):
            r.fill(x - 1, y - 1, x + 1, y + 1, CAVE)
            if x != x1 and (y == y1 or r.rng.random() < 0.5):
                x += 1 if x1 > x else -1
            elif y != y1:
                y += 1 if y1 > y else -1
    r.border(ROCK, 1)
    # entrance at the bottom
    r.fill(17, 26, 19, 28, CAVE)
    r.prune_unreachable((18, 27), ROCK)
    r.put(18, 27, spawn=True)
    r.put(18, 28, portal=(VERDANT, 40, 4, "cave", "Cave mouth — Verdant Reach"))
    # nodes on chamber edges
    def edge(x, y):
        return r.tile(x, y) == CAVE and any(r.inb(x + dx, y + dy) and r.tile(x + dx, y + dy) == ROCK
                                            for dx in (-1, 0, 1) for dy in (-1, 0, 1))
    cand = [(x, y) for y in range(2, 26) for x in range(2, 38) if edge(x, y) and r.free(x, y)]
    r.rng.shuffle(cand)
    wanted = [COPPER] * 4 + [IRON_VEIN] * 7 + [COAL] * 5 + [MUSHROOM] * 6 + [CRYSTAL] * 5
    got = []
    for (x, y) in cand:
        if not wanted:
            break
        if any(abs(px - x) <= 1 and abs(py - y) <= 1 for px, py in got):
            continue
        if abs(x - 18) <= 2 and y >= 24:
            continue
        r.put(x, y, obj=wanted.pop(0))
        got.append((x, y))
    r.scatter(2, 2, 38, 22, 0.012, npc=LURKER, tiles={CAVE}, min_gap=4)
    r.scatter(2, 2, 38, 26, 0.02, obj=BOULDER, tiles={CAVE})
    return r


# ============================================================================
# Region 4 — Wisp Warren (dungeon, 40x40)
# ============================================================================
def wisp_warren() -> Region:
    r = Region(WARREN, "Wisp Warren", 40, 40, WALL, "dungeon", 18)
    rooms = [(16, 32, 24, 37), (14, 22, 26, 28), (4, 22, 10, 30), (30, 20, 36, 30), (10, 8, 20, 16), (24, 4, 36, 14), (4, 4, 10, 12)]
    for (x0, y0, x1, y1) in rooms:
        r.fill(x0, y0, x1, y1, FLOOR)
    corridors = [((20, 32), (20, 28)), ((14, 25), (10, 25)), ((26, 25), (30, 25)), ((15, 22), (15, 16)),
                 ((33, 20), (30, 14)), ((20, 10), (24, 10)), ((10, 10), (10, 10)), ((7, 22), (7, 12))]
    for (a, b) in corridors:
        (x0, y0), (x1, y1) = a, b
        x, y = x0, y0
        while True:
            r.set_tile(x, y, FLOOR)
            if (x, y) == (x1, y1):
                break
            if x != x1:
                x += 1 if x1 > x else -1
            elif y != y1:
                y += 1 if y1 > y else -1
    r.border(WALL, 1)
    # entrance
    r.prune_unreachable((20, 37), WALL)
    r.put(20, 37, spawn=True)
    r.put(20, 38, portal=(VERDANT, 19, 67, "door", "Iron door — Verdant Reach"), tile=FLOOR)
    # crystals on room walls' inner edges, wisps in rooms, matron in the far NE room
    def edge(x, y):
        return r.tile(x, y) == FLOOR and any(r.inb(x + dx, y + dy) and r.tile(x + dx, y + dy) == WALL
                                             for dx in (-1, 0, 1) for dy in (-1, 0, 1))
    cand = [(x, y) for y in range(2, 37) for x in range(2, 38) if edge(x, y) and r.free(x, y)]
    r.rng.shuffle(cand)
    got = []
    for (x, y) in cand:
        if len(got) >= 14:
            break
        if any(abs(px - x) <= 2 and abs(py - y) <= 2 for px, py in got):
            continue
        r.put(x, y, obj=CRYSTAL)
        got.append((x, y))
    for (x0, y0, x1, y1) in rooms[1:6]:
        r.scatter(x0, y0, x1, y1, 0.05, npc=WISP, tiles={FLOOR}, min_gap=3)
    # hidden grove: ironwood + a deep pool in the NW room
    r.fill(5, 5, 9, 11, GRASS)
    r.put(5, 6, obj=IRONWOOD)
    r.put(9, 6, obj=IRONWOOD)
    r.put(5, 10, obj=IRONWOOD)
    r.put(8, 9, obj=DEEP_POOL, tile=SHALLOWS)
    r.put(7, 9, tile=SHALLOWS)
    r.put(9, 10, obj=MUSHROOM)
    # matron's chamber
    r.put(30, 8, npc=MATRON)
    r.put(27, 6, obj=CRYSTAL)
    r.put(34, 12, obj=CRYSTAL)
    r.scatter(2, 2, 38, 37, 0.01, obj=RUBBLE_PILE, tiles={FLOOR})
    return r


# ============================================================================
# Region 5 — Bandit Ridge (mountaintop, 30x30)
# ============================================================================
def bandit_ridge() -> Region:
    r = Region(RIDGE, "Bandit Ridge", 30, 30, SNOW, "mountain", 25)
    r.border(CLIFF, 2)
    # cliff bands forming a switchback climb from SW to the summit NE
    r.fill(2, 8, 20, 9, CLIFF)
    r.fill(9, 15, 27, 16, CLIFF)
    r.fill(2, 22, 20, 23, CLIFF)
    # gaps in the bands
    r.fill(21, 8, 24, 9, ROCK if False else SNOW)
    r.fill(5, 15, 8, 16, SNOW)
    r.fill(21, 22, 24, 23, SNOW)
    r.scatter(3, 3, 26, 26, 0.03, obj=BOULDER, tiles={SNOW})
    # arrival at the bottom-left
    r.put(3, 27, spawn=True)
    r.put(2, 27, portal=(VERDANT, 70, 6, "stairs_down", "Stairs down — Verdant Reach"), tile=SNOW)
    # camps on each terrace
    r.put(14, 25, obj=TENT); r.put(15, 26, obj=CAMPFIRE); r.put(13, 26, npc=BRUTE)
    r.put(10, 18, obj=TENT); r.put(11, 19, obj=CAMPFIRE)
    r.put(23, 12, obj=TENT); r.put(24, 13, obj=CAMPFIRE); r.put(22, 13, npc=BRUTE)
    r.scatter(3, 3, 26, 26, 0.012, npc=STALKER, tiles={SNOW}, min_gap=4)
    r.scatter(3, 17, 26, 21, 0.02, npc=BRUTE, tiles={SNOW}, min_gap=6)
    # summit: warlord's camp
    r.fill(6, 3, 18, 6, SNOW)
    r.put(9, 4, obj=TENT); r.put(15, 4, obj=TENT); r.put(12, 6, obj=CAMPFIRE)
    r.put(12, 4, npc=WARLORD)
    r.put(6, 4, obj=CRATE); r.put(18, 4, obj=BARREL)
    # a little coal and iron up here for the brave
    for (x, y, o) in [(4, 12, COAL), (26, 5, COAL), (4, 20, IRON_VEIN), (26, 19, IRON_VEIN)]:
        if r.free(x, y):
            r.put(x, y, obj=o)
    return r


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for build in (verdant_reach, shattered_coast, copper_hollow, wisp_warren, bandit_ridge):
        region = build()
        path = OUT / (region.name.lower().replace(" ", "_") + ".yaml")
        path.write_text(region.emit(), encoding="utf-8", newline="\n")
        print(f"wrote {path.relative_to(ROOT)} ({region.w}x{region.h})")


if __name__ == "__main__":
    main()
