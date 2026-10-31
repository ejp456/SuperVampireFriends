# Crimson Ledger — Co-op Vampire Noir Detective RPG (Bevy 0.15)

A *Disco Elysium*-inspired narrative detective RPG built in pure Rust with **Bevy 0.15**, featuring modular data-driven **Scene Classes** (`assets/scenes/case_01_scenes.ron`) linking rooms, alleys, and streets via interactive doorways, point-and-click A* exploration, 1-on-1 portrait dialogue, `2d6` skill checks, supernatural **Blood Sight**, live **Suspect Heartbeat** lie detection, and **Invite-Code Co-op Multiplayer** (Desktop TCP + Browser WebAssembly/WebSocket).

## Quick Start

> For full step-by-step setup instructions (prerequisites, Desktop build, WebAssembly browser server, and LAN/browser Co-op), see **[RUNNING_LOCALLY.md](RUNNING_LOCALLY.md)**.

```powershell
cargo run
```

Run all unit, A* pathfinding, invite-code, TCP netcode, and ECS schedule tests:

```powershell
cargo test
```

## Controls

| Input | Action |
| :--- | :--- |
| `Left / Right Click` (Scene) | **Point-and-click to move** (auto-pathfinds around walls & through doorways; clicking a clue/suspect walks over and inspects automatically) |
| `W A S D` / `Arrows` | Optional manual movement |
| `Tab` | Toggle **Blood Sight** (reveals scrubbed blood trails & suspect heartbeat rings) |
| `1` – `5` / `Left Click` | Pick dialogue lines or roll `[2d6 + Stat]` skill checks |
| `L` | Open / close the **Clue Ledger** |
| `M` | Open **Co-op Multiplayer** (Host an investigation or join with an Invite Code) |
| `F1` / `F2` / `F3` | Switch Detective Archetype (*Forensic Hound*, *Blood Seer*, *Midnight Enforcer*) |

## Co-op Multiplayer & Invite Codes

1. Press **`M`** (or click **`[M] Co-op`** in the bottom bar) and click **Host Investigation**.
2. Share the generated 10-character **`XXXXX-XXXXX` Invite Code** (click **Copy Invite Code**).
   - **Same PC (2 windows)**: Open a second game window, press **`M`**, and click **Quick Join Same PC (LOCAL)**.
   - **Friends on Wi-Fi / LAN / Tailscale / ZeroTier**: Enter the exact `XXXXX-XXXXX` Invite Code and press **`Enter`**.
   - **Across the open internet without VPN**: Forward TCP port `7878` on the host router (or connect via Tailscale so no router changes are needed).
3. Partner detectives appear live in the 2D crime scene with distinct trench coat tints, Blood Sight auras, synced suspect pulse spikes, and a shared real-time Clue Ledger!
