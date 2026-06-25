# V2 Surfpool Mainnet-Fork Lab

This stack runs `omnipair_v2` against a private Surfpool mainnet fork and exposes a small fork API for the V2 webapp. It is intentionally separate from the Helius-backed indexer path because private Surfpool transactions are not visible to Helius Atlas.

## Services

- `v2-surfpool-rpc`: private Surfpool RPC. Builds `anchor build -p omnipair-v2 -- --features "development"`, starts a mainnet fork, deploys `target/deploy/omnipair_v2.so`, and waits for the local program deployment log.
- `v2-surfpool-rpc-proxy`: public Solana RPC proxy for wallets. It forwards normal RPC calls and blocks unauthenticated `surfnet_*` cheatcodes. Set `FORK_ADMIN_TOKEN` for admin access.
- `v2-fork-api`: public V2 fork API. It bootstraps a fork-only META/USDC market by default, funds wallets through bounded Surfpool cheatcodes, serves webapp-compatible V2 read endpoints, and builds unsigned browser transactions.

## Local Commands

```sh
npm run v2-fork:surfpool
npm run v2-fork:rpc-proxy
npm run v2-fork:api
npm run test-surfpool-v2
npm run surfpool-v2-e2e
```

## Core Env

```sh
SURFPOOL_RPC_URL=http://127.0.0.1:8899
PUBLIC_SURFPOOL_RPC_URL=http://127.0.0.1:8898
OMNIPAIR_V2_PROGRAM_ID=358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv
OMNIPAIR_V2_BASE_MINT=METAwkXcqyXKy1AtsSgJ8JiUHwGCafnZL38n3vYmeta
OMNIPAIR_V2_QUOTE_MINT=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
FORK_ADMIN_TOKEN=<shared-secret>
```

The fork API accepts `FORK_LAB_PAYER_KEYPAIR_JSON`, `FORK_LAB_PAYER_KEYPAIR_BASE64`, `FORK_LAB_PAYER_KEYPAIR`, or `ANCHOR_WALLET`. If none are set it creates a local `.v2-fork-lab/payer.json`.

## API Endpoints

- `GET /health`
- `GET /api/v2/fork/config`
- `POST /api/v2/fork/fund-wallet`
- `POST /api/v2/fork/tx/add-liquidity`
- `POST /api/v2/fork/tx/swap`
- `POST /api/v2/fork/tx/deposit-collateral`
- `POST /api/v2/fork/tx/borrow`
- `POST /api/v2/fork/tx/repay`
- `POST /api/v2/fork/tx/open-hedge`
- `POST /api/v2/fork/tx/close-hedge`
- `GET /api/v2/markets`
- `GET /api/v2/markets/:marketAddress`
- `GET /api/v2/markets/:marketAddress/swaps`
- `GET /api/v2/users/:wallet/positions`
- `GET /api/v2/users/:wallet/activity`

Transaction endpoints return an unsigned base64 legacy transaction in `data.transaction`. The browser wallet signs and submits it to `data.rpcUrl`, which should be the public RPC proxy.
