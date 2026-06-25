#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

RPC_PORT="${SURFPOOL_RPC_PORT:-8899}"
WS_PORT="${SURFPOOL_WS_PORT:-8900}"
HOST="${SURFPOOL_HOST:-0.0.0.0}"
NETWORK="${SURFPOOL_NETWORK:-mainnet}"
LOG_PATH="${SURFPOOL_LOG_PATH:-/tmp/omnipair-v2-surfpool-logs}"
WALLET_PATH="${ANCHOR_WALLET:-deployer-keypair.json}"
PROGRAM_ID="${OMNIPAIR_V2_PROGRAM_ID:-358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv}"
DEPLOYMENT_TIMEOUT_SECONDS="${FORK_LAB_DEPLOYMENT_TIMEOUT_SECONDS:-180}"

if [[ "$RPC_PORT" != "8899" && "${FORK_LAB_ALLOW_NONSTANDARD_SURFPOOL_PORT:-false}" != "true" ]]; then
  cat >&2 <<EOF
Surfpool's generated Anchor deployment runbook currently targets http://127.0.0.1:8899.
Refusing to start the fork on port $RPC_PORT because local program upgrades can be skipped.

Use the default 8899 for the private v2-surfpool-rpc service. The API/proxy services can still
use Railway PORT. If you intentionally do not want local program deployment, set
FORK_LAB_ALLOW_NONSTANDARD_SURFPOOL_PORT=true.
EOF
  exit 1
fi

if [[ ! -f "$WALLET_PATH" ]]; then
  mkdir -p "$(dirname "$WALLET_PATH")"
  node -e "const { Keypair } = require('@solana/web3.js'); const fs = require('fs'); fs.writeFileSync(process.argv[1], JSON.stringify(Array.from(Keypair.generate().secretKey)))" "$WALLET_PATH"
fi

if [[ "${FORK_LAB_BUILD:-true}" != "false" ]]; then
  anchor build -p omnipair-v2 -- --features "development"
fi

for artifact in \
  target/deploy/omnipair_v2.so \
  target/deploy/omnipair_v2-keypair.json
do
  if [[ ! -f "$artifact" ]]; then
    echo "Missing required V2 Surfpool deployment artifact: $artifact" >&2
    echo "Run anchor build -p omnipair-v2 -- --features \"development\" before starting the fork." >&2
    exit 1
  fi
done

mkdir -p "$HOME/.config/solana"
cp "$WALLET_PATH" "$HOME/.config/solana/id.json"
solana config set --keypair "$HOME/.config/solana/id.json" >/dev/null

echo "Starting V2 Surfpool fork on ${HOST}:${RPC_PORT} with local artifact:"
ls -lh target/deploy/omnipair_v2.so

BOOT_LOG="$(mktemp -t omnipair-v2-surfpool-start.XXXXXX.log)"

cleanup() {
  if [[ -n "${SURFPOOL_PID:-}" ]] && kill -0 "$SURFPOOL_PID" 2>/dev/null; then
    kill "$SURFPOOL_PID" 2>/dev/null || true
    wait "$SURFPOOL_PID" 2>/dev/null || true
  fi
}

trap cleanup INT TERM

surfpool start \
  --network "$NETWORK" \
  --host "$HOST" \
  --port "$RPC_PORT" \
  --ws-port "$WS_PORT" \
  --no-tui \
  --no-studio \
  --yes \
  --legacy-anchor-compatibility \
  --airdrop-keypair-path "$WALLET_PATH" \
  --artifacts-path target/deploy \
  --log-path "$LOG_PATH" > >(tee "$BOOT_LOG") 2>&1 &

SURFPOOL_PID=$!
deadline=$((SECONDS + DEPLOYMENT_TIMEOUT_SECONDS))

while kill -0 "$SURFPOOL_PID" 2>/dev/null; do
  if grep -q "Runbook execution aborted" "$BOOT_LOG"; then
    echo "Surfpool deployment runbook aborted before the local V2 program was upgraded." >&2
    cleanup
    exit 1
  fi

  if grep -Eq "Program (Created|Upgraded) - Program ${PROGRAM_ID}" "$BOOT_LOG"; then
    echo "Surfpool fork is running local omnipair_v2 artifact for ${PROGRAM_ID}."
    wait "$SURFPOOL_PID"
    exit $?
  fi

  if (( SECONDS >= deadline )); then
    echo "Timed out waiting for Surfpool to deploy local V2 program artifact." >&2
    echo "Expected deploy log for ${PROGRAM_ID}." >&2
    cleanup
    exit 1
  fi

  sleep 1
done

wait "$SURFPOOL_PID"
