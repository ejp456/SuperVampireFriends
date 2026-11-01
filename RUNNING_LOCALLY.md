# Running Locally — Crimson Ledger (`SuperVampireFriends`)

This guide covers cloning the repository, installing prerequisites, and running **Crimson Ledger** locally in **Desktop (Native)** mode, **Browser (WebAssembly)** mode, and **Co-op Multiplayer**.

---

## 1. Prerequisites

1. **Git** — [https://git-scm.com/downloads](https://git-scm.com/downloads)
2. **Rust Toolchain (`1.85+`)** — Install via [rustup.rs](https://rustup.rs/):
   - **Windows**: Requires the *Desktop development with C++* workload (MSVC Build Tools / Windows SDK), which `rustup-init.exe` prompts to install automatically if missing.
   - **Linux (Ubuntu/Debian)**: Install system graphics/windowing headers required by Bevy:
     ```bash
     sudo apt-get update && sudo apt-get install -y pkg-config libx11-dev libasound2-dev libudev-dev libxkbcommon-x11-0 libwayland-dev
     ```
   - **macOS**: Install Xcode Command Line Tools (`xcode-select --install`).

Verify your Rust version:

```powershell
rustc --version
cargo --version
```

---

## 2. Clone the Repository

```powershell
git clone git@github.com:ejp456/SuperVampireFriends.git
cd SuperVampireFriends
```

*(Or via HTTPS: `git clone https://github.com/ejp456/SuperVampireFriends.git`)*

---

## 3. Option A — Run the Desktop Native Build (Fastest)

From the project root directory, launch the native Bevy window:

```powershell
cargo run
```

*(Note: The first build compiles Bevy 0.15 with `opt-level = 3` for smooth framerates and takes ~1–3 minutes; incremental rebuilds take only a few seconds. For a fully optimized release binary, run `cargo run --release`.)*

### Desktop Co-op Multiplayer
1. Press **`M`** (or click **`[M] Co-op`** in the bottom bar) and click **Host Investigation** (listens on TCP port `7878`).
2. To test solo on the **same PC**, open a second terminal, run `cargo run` again, press **`M`**, and click **Quick Join Same PC (LOCAL)**.
3. To play with friends on **Wi-Fi / LAN / Tailscale**, copy the 10-character **`XXXXX-XXXXX` Invite Code** from the host window, paste it into the join window, and press **`Enter`**.

---

## 4. Option B — Run in the Browser (WebAssembly + Co-op Relay Server)

The project includes a zero-dependency Rust HTTP + WebSocket Co-op Relay server (`src/bin/web_server.rs`) and a build script (`build_web.ps1`).

### One-Time WebAssembly Setup
Add the `wasm32-unknown-unknown` target and install the matching `wasm-bindgen-cli` version (`0.2.130`, matching `Cargo.toml`):

```powershell
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130
```

### Build & Serve (Windows PowerShell / WSL)

```powershell
# 1. Compile WASM client and generate web/pkg bindings
.\build_web.ps1

# 2a. Start the native Rust HTTP + WebSocket Co-op Relay server (default port 8080)
cargo run --release --bin web_server

# 2b. OR start via WSL / Linux (uses rustc if available, or falls back to Python 3):
wsl.exe bash ./run_wsl_server.sh
```

*(You can optionally pass a custom port, e.g. `cargo run --release --bin web_server -- 3000` or `wsl.exe bash ./run_wsl_server.sh 3000`.)*

### Build & Serve (macOS / Linux)

```bash
cargo build --release --target wasm32-unknown-unknown --bin vampire_noir_rpg
wasm-bindgen --out-dir web/pkg --target web --no-typescript target/wasm32-unknown-unknown/release/vampire_noir_rpg.wasm
./run_wsl_server.sh
```

### Play in Browser & Browser Co-op
1. Open **`http://127.0.0.1:8080`** in Chrome, Edge, or Firefox (requires WebGL2).
2. **Co-op in Browser**:
   - Open a second browser tab (or have another device on your LAN visit `http://<YOUR_LAN_IP>:8080`).
   - In Tab 1, press **`M`** → **Host Investigation** and copy the **`XXXXX-XXXXX` Invite Code** (or let Tab 2 click **Quick Join**).
   - Both browser players connect through `web_server`'s built-in WebSocket relay!

---

## 5. Controls Quick Reference

| Input | Action |
| :--- | :--- |
| `Left / Right Click` | Point-and-click movement & auto-inspect clues/suspects |
| `W A S D` / `Arrow Keys` | Manual character movement |
| `Tab` | Toggle **Blood Sight** (scrubbed blood trails & suspect heartbeat rings) |
| `1` – `5` / `Left Click` | Choose dialogue responses & roll `[2d6 + Stat]` skill checks |
| `L` | Open / close the **Clue Ledger** |
| `M` | Open **Co-op Multiplayer** menu |
| `F1` / `F2` / `F3` | Switch Detective Archetype (*Forensic Hound*, *Blood Seer*, *Midnight Enforcer*) |

---

## 6. Running Automated Tests

Run the full unit, A* pathfinding, invite-code, TCP netcode, and ECS schedule test suite:

```powershell
cargo test
```

---

## 7. Troubleshooting

- **`wasm-bindgen` version mismatch error**: Ensure `wasm-bindgen --version` reports `0.2.130` (`cargo install wasm-bindgen-cli --version 0.2.130 --force`).
- **Port `8080` already in use**: Pass another port to the web server, e.g. `cargo run --release --bin web_server -- 8081`, then open `http://127.0.0.1:8081`.
- **LAN Co-op can't connect on Windows**: Allow `vampire_noir_rpg.exe` (TCP `7878`) or `web_server.exe` (TCP `8080`) through Windows Defender Firewall for Private networks, or connect over [Tailscale](https://tailscale.com/).

---

## 8. Automated Google Cloud Post-Submit Deploy (`streaming-friends-server`)

Enable the repository's `.githooks/post-commit` and `.githooks/pre-push` hooks on any clone so committing or pushing on `main` automatically compiles the WebAssembly bundle, deploys atomically to `/opt/crimson-ledger` on GCE, restarts `crimson-ledger.service`, and verifies live HTTP + SHA-256 + WebSocket health:

```bash
./scripts/install_hooks.sh
```

- **Manual deploy & live verification**: `./scripts/deploy_and_verify.sh` (pass `--force` to redeploy even if the current commit SHA is already live)
- **Skip deploy for a single commit/push**: `SKIP_DEPLOY=1 git commit -m "..."` or `SKIP_DEPLOY=1 git push`
