# Crimson Ledger — Co-op Supernatural Noir Detective RPG (Bevy 0.15)

A *Disco Elysium*-inspired narrative detective RPG built in pure Rust with **Bevy 0.15**, featuring **4 Selectable Supernatural Detectives**, modular data-driven **Scene Classes** (`assets/scenes/case_01_scenes.ron`), point-and-click A* exploration, 1-on-1 portrait dialogue, `2d6` skill checks, character-specific supernatural senses, and **Invite-Code Co-op Multiplayer** (Desktop TCP + Browser WebAssembly/WebSocket).

## Playable Supernatural Detectives

| Hotkey | Detective | Nature | `[Tab]` Sense & Special Abilities |
| :--- | :--- | :--- | :--- |
| **`1` / `F1`** | **Detective Richey** | **Vampire** | **Vampire Senses**: Sees scrubbed crimson blood trails & live suspect heartbeat (`BPM`) lie rings; reads arterial blood-iron chemistry & uses Crimson Dread (`THIRST` overdrive: `+SAN`, `+PUL`, `-DED`). |
| **`2` / `F2`** | **Det. Lucker & Ron** | **Ghost (`Det. Lucker`) + Possessed Human (`Ron`)** | **Phantom Sight**: Ghost Detective Lucker rides shotgun inside nervous human host Ron—reveals cyan death echoes & guilt tremors, channels **Corpse Séances** through Ron's voice, and booms with **Ghost Possession** (`SYNC` overdrive: `+DED`, `+NOC`, `-PUL`). |
| **`3` / `F3`** | **Detective Pinckney** | **Werewolf** | **Lupine Scent**: Tracks golden-amber scent plumes (cordite, perfume, boot-spoor) through harbor rain, smells fear-sweat adrenaline spikes, moves with **Fleet Stride**, and unleashes **Werewolf Snarl** (`FURY` overdrive: `+2 PUL`, `+SAN`, `-DED`, `-NOC`). |
| **`4` / `F4`** | **Detective Graves** | **Zombie Revenant** | **Mortis Sight**: Stitched morgue revenant with **Zombie Psychometry** (touches wounds to replay tissue/lividity memory), painless bullet-proof nerve in standoffs, and cold morgue clarity (`DECAY` overdrive: `+DED`, `+SAN`, `-NOC`). |

## Quick Start

> For full step-by-step setup instructions (prerequisites, Desktop build, WebAssembly browser server, and LAN/browser Co-op), see **[RUNNING_LOCALLY.md](RUNNING_LOCALLY.md)**.

```powershell
cargo run
```

Run all unit, character ability, A* pathfinding, invite-code, TCP netcode, and ECS schedule tests:

```powershell
cargo test
```

## Controls

| Input | Action |
| :--- | :--- |
| `C` / `F1`–`F4` | Open **Character Selection** overlay or quick-switch between the 4 detectives |
| `Left / Right Click` (Scene) | **Point-and-click to move** (auto-pathfinds around walls & through doorways; clicking a clue/suspect walks over and inspects automatically) |
| `W A S D` / `Arrows` | Optional manual movement |
| `Tab` | Toggle your detective's **Supernatural Sense** (*Vampire Senses*, *Phantom Sight*, *Lupine Scent*, or *Mortis Sight*) |
| `1` – `5` / `Left Click` | Pick dialogue lines, use character powers, or roll `[2d6 + Stat]` skill checks |
| `L` | Open / close the **Clue Ledger** |
| `M` | Open **Co-op Multiplayer** (Host an investigation or join with an Invite Code) |
