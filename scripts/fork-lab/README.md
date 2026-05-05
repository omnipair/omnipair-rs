# Omnipair Fork Lab

This directory contains the fork-only services used to drive the webapp against
mainnet state while executing this branch's local Omnipair and
`leverage_delegate` programs.

## Services

- `fork-lab:surfpool` starts Surfpool on a mainnet fork and deploys local
  artifacts from `target/deploy`.
- `fork-lab:rpc-proxy` forwards normal Solana RPC traffic to Surfpool and blocks
  public `surfnet_*` cheatcodes.
- `fork-lab:api` exposes branch-aware helper endpoints for wallet funding,
  building unsigned leverage transactions, reading fork state, and running the
  delegated take-profit keeper path.

## Local Run

Terminal 1:

```bash
anchor build -- --features "development"
npm run fork-lab:surfpool
```

Terminal 2:

```bash
FORK_ADMIN_TOKEN=dev-secret npm run fork-lab:rpc-proxy
```

Terminal 3:

```bash
SURFPOOL_RPC_URL=http://127.0.0.1:8899 \
PUBLIC_SURFPOOL_RPC_URL=http://127.0.0.1:8898 \
FORK_ADMIN_TOKEN=dev-secret \
npm run fork-lab:api
```

Then point the webapp at:

```env
NEXT_PUBLIC_FORK_MODE=true
NEXT_PUBLIC_FORK_API_URL=http://127.0.0.1:3011
NEXT_PUBLIC_HELIUS_RPC_URL=http://127.0.0.1:8898
NEXT_PUBLIC_OMNIPAIR_API_URL=http://127.0.0.1:3011/api/v1
NEXT_PUBLIC_LEVERAGE_ENABLED=true
```

## Railway Shape

Create three Railway services from this repo:

Railway should deploy this branch with the root `Dockerfile`. That image pins
Node/npm, Solana, Anchor, and Surfpool so the service start commands do not
depend on Railway autodetection. If a service is already crashing with
`npm: command not found`, redeploy after pulling this branch update and confirm
the service build logs say it is using the Dockerfile.

For cleaner rebuilds, point each Railway service at its service-specific config
file instead of the root `railway.json`:

- `surfpool-rpc`: `/railway/surfpool-rpc.json`
- `surfpool-rpc-proxy`: `/railway/rpc-proxy.json`
- `fork-api`: `/railway/fork-api.json`

Those config files set service-specific watch patterns. For example,
`scripts/fork-lab/api.ts` changes rebuild only `fork-api`, and
`scripts/fork-lab/rpc_proxy.ts` changes rebuild only the proxy. The API and
proxy also use the lightweight `Dockerfile.fork-lab-node`; only `surfpool-rpc`
builds the heavier Anchor/Surfpool image.

1. `surfpool-rpc`
   - start command: `npm run fork-lab:surfpool`
   - expose the RPC port as the service `$PORT`
   - set `SURFPOOL_WS_PORT=8900`
2. `surfpool-rpc-proxy`
   - start command: `npm run fork-lab:rpc-proxy`
   - set `SURFPOOL_RPC_URL` to the private URL for `surfpool-rpc`
   - set `FORK_ADMIN_TOKEN`
3. `fork-api`
   - start command: `npm run fork-lab:api`
   - set `SURFPOOL_RPC_URL` to the private `surfpool-rpc` URL
   - set `PUBLIC_SURFPOOL_RPC_URL` to the public proxy URL
   - set `FORK_ADMIN_TOKEN`
   - provide `ANCHOR_WALLET` or `FORK_LAB_PAYER_KEYPAIR` for the keeper/payer

Only expose the proxy and API to the browser. Keep raw Surfpool private because
it accepts fork mutation cheatcodes.

`/api/v1/fork/fund-wallet` is public-but-bounded by default: it can only fund
the requested wallet with the selected pair's two tokens and SOL on the fork.
Set `FORK_ALLOW_PUBLIC_FUNDING=false` plus `FORK_ADMIN_TOKEN` if you want to
require an admin header for funding too.
