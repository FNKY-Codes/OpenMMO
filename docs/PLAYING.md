# Playing OpenMMO

## Install (Windows)

1. Download the latest `openmmo-client-*-windows-x64.zip` from the
   [Releases page](https://github.com/FNKY-Codes/OpenMMO/releases).
2. Unzip it anywhere. Keep `openmmo-client.exe`, `content/` and `assets/`
   together in the same folder.
3. Run `openmmo-client.exe`.

Windows SmartScreen may warn that the app is unrecognised because the build is
not code-signed. Click **More info → Run anyway**.

## Logging in

The login window is pre-filled with the official server.

- **Username / Password** — your account. The first time you use a username the
  account is created with whatever password you type, so pick a good one; there
  is no password reset yet.
- **Character name** — your in-game name. A character belongs to the account
  that created it. Logging in with the same character from a second device
  disconnects the first.

Your character is saved automatically every minute and when you log out.

## The screen

- **Top-left** — what the thing under your cursor will do: *Chop Birch
  Timber*, *Attack Mutant Rat (level 2)*, *Open Bank Chest*, *Walk here*.
- **Top-right** — minimap (click it to walk), compass, your HP orb and the
  region name. Yellow dots are hostile NPCs, blue are people to talk to,
  white are players, red are dropped items.
- **Bottom-right** — the side panel. Tabs: Inventory, Worn Equipment, Skills,
  Quest Journal, Friends & Trade, Settings.
- **Bottom-left** — chat. Tabs filter game messages, player chat, clan and
  private messages. Pick a channel next to the input box; choose *PM* to
  whisper someone by name.

## Controls

- **Left-click** a tile to walk, a tree/rock/pool to gather, an NPC to attack
  or talk, a chest/furnace/anvil/workbench/cookfire/trade board to use it.
- **Right-click** anything for the full menu (*Walk here*, *Examine*, …).
- **Inventory**: left-click food to eat it and gear to wield it; right-click
  for *Drop*, *Examine* and the rest. Hover for stats.
- **Middle-drag** rotates the camera; **scroll** zooms.
- In a dialogue, press **1–9** to pick an option.

## Getting started

Talk to the **Wasteland Guide** by the well in the town square — he'll start
you on *First Steps in the Reach*. You begin with a Scrap Club, a Training
Axe, a Stone Pick, five Cooked Minnows and 30 Scrap.

- **Scrap** is money. Marla's General Store (north-west of the square) buys
  almost anything and sells tools and food. Grist's Smithy (north-east) sells
  bronze gear.
- **Birch woods** are west of town. Chop timber, saw it into planks at the
  workbenches on the west side of the square.
- **Copper Ridge** is north. Mine copper, smelt it at the furnaces by the
  smithy, hammer ingots into gear on the anvils.
- **Still pools** are south. Fish minnows (buy a rod from Marla), cook them at
  the cookfires.
- **Rats** roam the outskirts and won't attack first. Tigers in the woods and
  bandits in the fenced camps to the east will.
- The **bank** is the building south of the square. The **trade board** beside
  it lets you post buy/sell offers to other players.

Further afield, for when you're stronger: the cave mouth in the north cliffs
(Copper Hollow, iron and coal, level 12+), the iron door in the south-west
ruins (Wisp Warren, level 18+), the stairs in the north-east cliffs (Bandit
Ridge, level 25+, where the Warlord waits) and the road east to the Shattered
Coast (level 15+, trout, oaks, marksmen).

## Troubleshooting

- **"Connection failed"** — the server may be restarting; wait 30 s and retry.
  If it persists, check the server URL still ends in `/ws`.
- **Black window / no world** — the `content/` and `assets/` folders must sit
  beside the exe. Re-extract the zip.
- **Logs** — run from a terminal with `RUST_LOG=info` to see what the client is
  doing.
