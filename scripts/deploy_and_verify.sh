#!/usr/bin/env bash
# ==============================================================================
# Crimson Ledger (SuperVampireFriends) — Automated GCloud Deploy & Live Verifier
# Builds the WebAssembly bundle, deploys atomically to streaming-friends-server
# (/opt/crimson-ledger on GCE), restarts crimson-ledger.service, and verifies
# live HTTP + SHA-256 + WebSocket co-op relay health on http://34.130.176.244.
# ==============================================================================
set -euo pipefail

if [ "${SKIP_DEPLOY:-0}" = "1" ]; then
  echo "[post-submit] SKIP_DEPLOY=1 set — skipping GCloud deployment."
  exit 0
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"

GCP_PROJECT_ID="${GCP_PROJECT_ID:-streaming-friends-server}"
GCP_ZONE="${GCP_ZONE:-northamerica-northeast2-b}"
GCP_VM_NAME="${GCP_VM_NAME:-instance-20261031-205001}"
GCP_SSH_USER="${GCP_SSH_USER:-etai_pinckney}"
GCP_APP_OWNER="${GCP_APP_OWNER:-etaipinckney}"
GCP_DEPLOY_DIR="${GCP_DEPLOY_DIR:-/opt/crimson-ledger}"
GCP_PUBLIC_HOST="${GCP_PUBLIC_HOST:-34.130.176.244}"
GCP_PUBLIC_PORT="${GCP_PUBLIC_PORT:-80}"

FORCE_DEPLOY=0
for arg in "$@"; do
  case "$arg" in
    --force|-f) FORCE_DEPLOY=1 ;;
  esac
done

if ! command -v gcloud >/dev/null 2>&1 && [ -x "$HOME/google-cloud-sdk/bin/gcloud" ]; then
  export PATH="$HOME/google-cloud-sdk/bin:$PATH"
fi

if ! command -v gcloud >/dev/null 2>&1; then
  echo "[post-submit] Error: gcloud CLI not found in PATH or ~/google-cloud-sdk/bin." >&2
  exit 1
fi

HEAD_SHA="$(git -C "$REPO_ROOT" rev-parse HEAD)"
SHORT_SHA="$(git -C "$REPO_ROOT" rev-parse --short HEAD)"
if ! git -C "$REPO_ROOT" diff --quiet -- src Cargo.toml Cargo.lock web assets scripts/wsl_server.py 2>/dev/null; then
  DEPLOY_ID="${HEAD_SHA}-dirty"
else
  DEPLOY_ID="${HEAD_SHA}"
fi

LIVE_BASE_URL="http://${GCP_PUBLIC_HOST}"
if [ "$GCP_PUBLIC_PORT" != "80" ]; then
  LIVE_BASE_URL="http://${GCP_PUBLIC_HOST}:${GCP_PUBLIC_PORT}"
fi

# Fast idempotency check: skip rebuild if this exact clean commit is already live
if [ "$FORCE_DEPLOY" -eq 0 ] && [ "$DEPLOY_ID" = "$HEAD_SHA" ]; then
  LIVE_INFO="$(curl -fsSL --max-time 3 "${LIVE_BASE_URL}/BUILD_INFO.txt" 2>/dev/null || true)"
  if printf '%s\n' "$LIVE_INFO" | grep -q "^commit=${DEPLOY_ID}$"; then
    echo "[post-submit] Commit ${SHORT_SHA} is already live & verified at ${LIVE_BASE_URL} — nothing to do."
    exit 0
  fi
fi

echo "=============================================================================="
echo "  [post-submit] Deploying SuperVampireFriends (${SHORT_SHA}) -> ${GCP_PROJECT_ID}"
echo "=============================================================================="

# ------------------------------------------------------------------------------
# 1. Build WebAssembly game binary + JS bindings via native Linux toolchain
# ------------------------------------------------------------------------------
for bin_req in cargo cc wasm-bindgen; do
  if ! command -v "$bin_req" >/dev/null 2>&1; then
    echo "[post-submit] Error: Required Linux build tool '$bin_req' not found in PATH." >&2
    exit 1
  fi
done

mkdir -p "$REPO_ROOT/web/pkg"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/crimson-ledger-target}"
mkdir -p "$CARGO_TARGET_DIR"

echo "==> [1/4] Compiling WASM via native Linux Rust toolchain ($CARGO_TARGET_DIR)..."
(
  cd "$REPO_ROOT"
  cargo build --release --target wasm32-unknown-unknown --bin vampire_noir_rpg
  wasm-bindgen \
    --out-dir "$REPO_ROOT/web/pkg" \
    --target web \
    --no-typescript \
    --remove-name-section \
    --remove-producers-section \
    "$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/vampire_noir_rpg.wasm"
)

gzip -9 -k -f "$REPO_ROOT/web/pkg/vampire_noir_rpg_bg.wasm" "$REPO_ROOT/web/pkg/vampire_noir_rpg.js"

WASM_SHA256="$(sha256sum "$REPO_ROOT/web/pkg/vampire_noir_rpg_bg.wasm" | awk '{print $1}')"
WASM_BYTES="$(wc -c < "$REPO_ROOT/web/pkg/vampire_noir_rpg_bg.wasm" | tr -d ' ')"
BUILT_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# ------------------------------------------------------------------------------
# 2. Stage & package deployment archive
# ------------------------------------------------------------------------------
echo "==> [2/4] Packaging deployment archive (WASM size: ${WASM_BYTES} bytes, sha256: ${WASM_SHA256:0:12}...)..."
STAGE_LOCAL="$(mktemp -d /tmp/svf-local-stage.XXXXXX)"
ARCHIVE_LOCAL="/tmp/svf-deploy-${SHORT_SHA}-$$.tar.gz"
trap 'rm -rf "$STAGE_LOCAL" "$ARCHIVE_LOCAL"' EXIT

mkdir -p "$STAGE_LOCAL/scripts"
cp -r "$REPO_ROOT/web" "$STAGE_LOCAL/web"
cp -r "$REPO_ROOT/assets" "$STAGE_LOCAL/assets"
cp "$REPO_ROOT/run_wsl_server.sh" "$STAGE_LOCAL/run_wsl_server.sh"
cp "$REPO_ROOT/scripts/wsl_server.py" "$STAGE_LOCAL/scripts/wsl_server.py"
chmod 0755 "$STAGE_LOCAL/run_wsl_server.sh" "$STAGE_LOCAL/scripts/wsl_server.py"

cat > "$STAGE_LOCAL/web/BUILD_INFO.txt" <<EOF
commit=${DEPLOY_ID}
short_sha=${SHORT_SHA}
wasm_sha256=${WASM_SHA256}
wasm_bytes=${WASM_BYTES}
built_at=${BUILT_AT}
EOF
cp "$STAGE_LOCAL/web/BUILD_INFO.txt" "$STAGE_LOCAL/BUILD_INFO.txt"

tar -C "$STAGE_LOCAL" -czf "$ARCHIVE_LOCAL" .

# ------------------------------------------------------------------------------
# 3. Upload & atomically swap on Google Cloud Compute Engine
# ------------------------------------------------------------------------------
TARGET_HOST="${GCP_VM_NAME}"
if [ -n "$GCP_SSH_USER" ]; then
  TARGET_HOST="${GCP_SSH_USER}@${GCP_VM_NAME}"
fi
REMOTE_ARCHIVE="/tmp/svf-deploy-$$.tar.gz"

echo "==> [3/4] Uploading & installing atomically to ${TARGET_HOST}:${GCP_DEPLOY_DIR} ..."
gcloud compute scp \
  --project="$GCP_PROJECT_ID" \
  --zone="$GCP_ZONE" \
  --ssh-key-expire-after=10m \
  --quiet \
  "$ARCHIVE_LOCAL" \
  "${TARGET_HOST}:${REMOTE_ARCHIVE}"

gcloud compute ssh \
  --project="$GCP_PROJECT_ID" \
  --zone="$GCP_ZONE" \
  --ssh-key-expire-after=10m \
  --quiet \
  "$TARGET_HOST" \
  --command "DEPLOY_DIR=$(printf '%q' "$GCP_DEPLOY_DIR") APP_OWNER=$(printf '%q' "$GCP_APP_OWNER") REMOTE_ARCHIVE=$(printf '%q' "$REMOTE_ARCHIVE") bash -s" << 'EOF'
set -euo pipefail

STAGE_DIR="/tmp/svf-remote-stage-$$"
mkdir -p "$STAGE_DIR"
tar -xzf "$REMOTE_ARCHIVE" -C "$STAGE_DIR"
rm -f "$REMOTE_ARCHIVE"

sudo mkdir -p "$DEPLOY_DIR/scripts"

for subdir in web assets; do
  sudo rm -rf "$DEPLOY_DIR/${subdir}.next"
  sudo mv "$STAGE_DIR/${subdir}" "$DEPLOY_DIR/${subdir}.next"
  sudo rm -rf "$DEPLOY_DIR/${subdir}.prev"
  if [ -d "$DEPLOY_DIR/${subdir}" ]; then
    sudo mv "$DEPLOY_DIR/${subdir}" "$DEPLOY_DIR/${subdir}.prev"
  fi
  sudo mv "$DEPLOY_DIR/${subdir}.next" "$DEPLOY_DIR/${subdir}"
  sudo rm -rf "$DEPLOY_DIR/${subdir}.prev"
done

sudo install -m 0755 "$STAGE_DIR/run_wsl_server.sh" "$DEPLOY_DIR/run_wsl_server.sh.next"
sudo mv -f "$DEPLOY_DIR/run_wsl_server.sh.next" "$DEPLOY_DIR/run_wsl_server.sh"

sudo install -m 0755 "$STAGE_DIR/scripts/wsl_server.py" "$DEPLOY_DIR/scripts/wsl_server.py.next"
sudo mv -f "$DEPLOY_DIR/scripts/wsl_server.py.next" "$DEPLOY_DIR/scripts/wsl_server.py"

sudo cp "$STAGE_DIR/BUILD_INFO.txt" "$DEPLOY_DIR/BUILD_INFO.txt"

if id "$APP_OWNER" >/dev/null 2>&1; then
  sudo chown -R "${APP_OWNER}:${APP_OWNER}" "$DEPLOY_DIR"
fi
sudo chmod -R a+rX "$DEPLOY_DIR"
rm -rf "$STAGE_DIR"

sudo systemctl restart crimson-ledger.service
sudo systemctl is-active --quiet crimson-ledger.service
EOF

# ------------------------------------------------------------------------------
# 4. End-to-End Live Verification against http://34.130.176.244
# ------------------------------------------------------------------------------
echo "==> [4/4] Verifying live deployment at ${LIVE_BASE_URL} ..."
python3 - "$GCP_PUBLIC_HOST" "$GCP_PUBLIC_PORT" "$DEPLOY_ID" "$WASM_SHA256" "$WASM_BYTES" << 'PYEOF'
import base64
import socket
import sys
import time
import urllib.request

host = sys.argv[1]
port = int(sys.argv[2])
expected_commit = sys.argv[3]
expected_sha256 = sys.argv[4]
expected_bytes = int(sys.argv[5])

base_url = f"http://{host}" if port == 80 else f"http://{host}:{port}"

# 1. Poll /BUILD_INFO.txt (up to 10s after service restart)
build_info = ""
for attempt in range(10):
    try:
        with urllib.request.urlopen(f"{base_url}/BUILD_INFO.txt", timeout=4) as resp:
            if resp.status == 200:
                build_info = resp.read().decode("utf-8")
                if f"commit={expected_commit}" in build_info:
                    break
    except Exception:
        pass
    time.sleep(1)
else:
    raise SystemExit(f"Verification FAILED: /BUILD_INFO.txt did not match commit={expected_commit}.\nGot:\n{build_info}")

if f"wasm_sha256={expected_sha256}" not in build_info:
    raise SystemExit(f"Verification FAILED: wasm_sha256 mismatch in /BUILD_INFO.txt:\n{build_info}")
print(f"  [OK] /BUILD_INFO.txt verified (commit={expected_commit[:12]}, wasm_sha256={expected_sha256[:12]})")

# 2. Verify index.html (GET /)
with urllib.request.urlopen(f"{base_url}/", timeout=5) as resp:
    html = resp.read().decode("utf-8", errors="replace")
    assert resp.status == 200 and "bevy-canvas" in html, "index.html check failed"
print("  [OK] GET / (index.html HTTP 200 + #bevy-canvas present)")

# 3. Verify WASM binary delivery headers (HEAD /pkg/vampire_noir_rpg_bg.wasm)
req = urllib.request.Request(f"{base_url}/pkg/vampire_noir_rpg_bg.wasm", method="HEAD")
with urllib.request.urlopen(req, timeout=5) as resp:
    ctype = resp.headers.get("Content-Type", "")
    clen = int(resp.headers.get("Content-Length", "0"))
    assert resp.status == 200, f"Unexpected status {resp.status}"
    assert "application/wasm" in ctype, f"Unexpected Content-Type: {ctype}"
    assert clen == expected_bytes, f"Content-Length {clen} != expected {expected_bytes}"
print(f"  [OK] HEAD /pkg/vampire_noir_rpg_bg.wasm (HTTP 200, application/wasm, {clen:,} bytes)")

# 4. Verify live WebSocket Co-op Relay upgrade & room creation (/ws)
ws_key = base64.b64encode(b"svf-verify-probe").decode("ascii")
with socket.create_connection((host, port), timeout=5) as sock:
    handshake = (
        f"GET /ws HTTP/1.1\r\n"
        f"Host: {host}:{port}\r\n"
        f"Upgrade: websocket\r\n"
        f"Connection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {ws_key}\r\n"
        f"Sec-WebSocket-Version: 13\r\n\r\n"
    ).encode("utf-8")
    sock.sendall(handshake)
    resp_hdr = sock.recv(1024).decode("utf-8", errors="replace")
    if "101 Switching Protocols" not in resp_hdr:
        raise SystemExit(f"Verification FAILED: /ws did not upgrade to WebSocket:\n{resp_hdr}")

    # Send masked client text frame: "HOST|VERIFY-CHK"
    payload = b"HOST|VERIFY-CHK"
    mask = b"\x12\x34\x56\x78"
    masked = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
    frame = bytes([0x81, 0x80 | len(payload)]) + mask + masked
    sock.sendall(frame)
    ws_reply = sock.recv(256)
    if b"HOSTED|VERIFY-CHK" not in ws_reply:
        raise SystemExit(f"Verification FAILED: /ws relay did not reply HOSTED|VERIFY-CHK: {ws_reply!r}")
print("  [OK] WS /ws (101 Switching Protocols + live co-op room relay verified)")
PYEOF

echo "=============================================================================="
echo "  [SUCCESS] Live & verified at ${LIVE_BASE_URL} (commit ${SHORT_SHA})"
echo "=============================================================================="
