#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

RPC_PORT="${PORT:-${SURFPOOL_RPC_PORT:-8899}}"
WS_PORT="${SURFPOOL_WS_PORT:-8900}"
HOST="${SURFPOOL_HOST:-0.0.0.0}"
NETWORK="${SURFPOOL_NETWORK:-mainnet}"
LOG_PATH="${SURFPOOL_LOG_PATH:-/tmp/omnipair-surfpool-logs}"
WALLET_PATH="${ANCHOR_WALLET:-deployer-keypair.json}"

if [[ "${FORK_LAB_BUILD:-true}" != "false" ]]; then
  anchor build -- --features "development"
fi

exec surfpool start \
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
  --log-path "$LOG_PATH"
