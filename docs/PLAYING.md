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

The login window is pre-filled with the official server URL.

- **Username / Password** — your account. The first time you use a username the
  account is created with whatever password you type, so pick a good one; there
  is no password reset yet.
- **Character Name** — your in-game name. A character belongs to the account
  that created it. Logging in with the same character from a second device
  disconnects the first.

Your character is saved automatically every minute and when you log out.

## Controls

- **Left-click** a tile to walk, an NPC to talk/attack, an object to harvest.
- **Right-click** for the context menu (attack, trade, follow, examine, …).
- **Scroll / drag** to move the camera.
- Side panel tabs: Inventory, Bank, Skills, Quests, Market, Friends.
- Chat box at the bottom; choose Local / Global / Clan from the dropdown.

## Troubleshooting

- **"Connection failed"** — the server may be restarting; wait 30 s and retry.
  If it persists, check the server URL still ends in `/ws`.
- **Black window / no world** — the `content/` and `assets/` folders must sit
  beside the exe. Re-extract the zip.
- **Logs** — run from a terminal with `RUST_LOG=info` to see what the client is
  doing.
