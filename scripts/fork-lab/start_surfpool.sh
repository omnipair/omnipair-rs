#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

RPC_PORT="${SURFPOOL_RPC_PORT:-8899}"
WS_PORT="${SURFPOOL_WS_PORT:-8900}"
HOST="${SURFPOOL_HOST:-0.0.0.0}"
NETWORK="${SURFPOOL_NETWORK:-mainnet}"
LOG_PATH="${SURFPOOL_LOG_PATH:-/tmp/omnipair-surfpool-logs}"
WALLET_PATH="${ANCHOR_WALLET:-deployer-keypair.json}"
OMNIPAIR_PROGRAM_ID="omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE"
LEVERAGE_DELEGATE_PROGRAM_ID="EPGF9iFrbGnhWgC3To9rC9vxinEYuDHaz4RXgLPvuRkp"
DEPLOYMENT_TIMEOUT_SECONDS="${FORK_LAB_DEPLOYMENT_TIMEOUT_SECONDS:-180}"

if [[ "$RPC_PORT" != "8899" && "${FORK_LAB_ALLOW_NONSTANDARD_SURFPOOL_PORT:-false}" != "true" ]]; then
  cat >&2 <<EOF
Surfpool's generated Anchor deployment runbook currently targets http://127.0.0.1:8899.
Refusing to start the fork on port $RPC_PORT because local program upgrades would be skipped,
leaving the fork on mainnet program bytes and causing InstructionFallbackNotFound for leverage txs.

Use the default 8899 for the surfpool-rpc service. The API/proxy services can still use Railway PORT.
If you intentionally do not want local program deployment, set FORK_LAB_ALLOW_NONSTANDARD_SURFPOOL_PORT=true.
EOF
  exit 1
fi

if [[ "${FORK_LAB_BUILD:-true}" != "false" ]]; then
  anchor build -- --features "development"
fi

for artifact in \
  target/deploy/omnipair.so \
  target/deploy/omnipair-keypair.json \
  target/deploy/leverage_delegate.so \
  target/deploy/leverage_delegate-keypair.json
do
  if [[ ! -f "$artifact" ]]; then
    echo "Missing required Surfpool deployment artifact: $artifact" >&2
    echo "Run anchor build -- --features \"development\" before starting the fork." >&2
    exit 1
  fi
done

echo "Starting Surfpool fork on ${HOST}:${RPC_PORT} with local artifacts:"
ls -lh target/deploy/omnipair.so target/deploy/leverage_delegate.so

BOOT_LOG="$(mktemp -t omnipair-surfpool-start.XXXXXX.log)"

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
    echo "Surfpool deployment runbook aborted before local programs were upgraded." >&2
    cleanup
    exit 1
  fi

  if grep -q "Program Upgraded - Program ${OMNIPAIR_PROGRAM_ID}" "$BOOT_LOG" \
    && grep -Eq "Program (Created|Upgraded) - Program ${LEVERAGE_DELEGATE_PROGRAM_ID}" "$BOOT_LOG"
  then
    echo "Surfpool fork is running local Omnipair and leverage delegate artifacts."
    wait "$SURFPOOL_PID"
    exit $?
  fi

  if (( SECONDS >= deadline )); then
    echo "Timed out waiting for Surfpool to deploy local program artifacts." >&2
    echo "Expected Omnipair upgrade log for ${OMNIPAIR_PROGRAM_ID} and delegate deploy log for ${LEVERAGE_DELEGATE_PROGRAM_ID}." >&2
    cleanup
    exit 1
  fi

  sleep 1
done

wait "$SURFPOOL_PID"
