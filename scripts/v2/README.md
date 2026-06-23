# V2 Devnet Helpers

These scripts create a disposable V2 devnet market and tester balances without committing keypairs.

Local state and generated keypairs live in `~/.config/omnipair/v2-devnet` by default. Override with `OMNIPAIR_V2_DEVNET_CONFIG_DIR` or `OMNIPAIR_V2_DEVNET_STATE`.

```bash
export ANCHOR_PROVIDER_URL=https://api.devnet.solana.com
export ANCHOR_WALLET=~/.config/solana/id.json
export OMNIPAIR_V2_PROGRAM_ID=oMNi2XGwWxDbEvhS2pWRQ6dtw8GkNBV42hfLZD6WmMF
export OMNIPAIR_V2_PROGRAM_KEYPAIR=~/.config/omnipair/v2-devnet/omni2-v2-program-keypair.json

yarn v2:build-devnet
yarn v2:deploy-devnet
yarn v2:create-mock-tokens
yarn v2:mint-mock-tokens <tester-wallet>
yarn v2:bootstrap-market
yarn v2:smoke-devnet
```

Devnet currently has SBPFv3 deployment active and rejects SBPF v0/v1/v2
program deployments. Use `yarn v2:build-devnet` before deploying so the
artifact is built with `--arch v3`.

`yarn v2:deploy-devnet` needs the deployer wallet to hold enough devnet SOL for
program rent. The generated vanity program keypair stays outside git at
`~/.config/omnipair/v2-devnet/omni2-v2-program-keypair.json` unless
`OMNIPAIR_V2_PROGRAM_KEYPAIR` points elsewhere.

Useful knobs:

- `OMNIPAIR_V2_TOKEN_PROGRAM=token2022` creates Token-2022 mock mints.
- `OMNIPAIR_V2_MOCK_DECIMALS=6` controls mock mint decimals.
- `OMNIPAIR_V2_MINT_AMOUNT=1000000` controls tester faucet size in human units.
- `OMNIPAIR_V2_BASE_LIQUIDITY=100000` and `OMNIPAIR_V2_QUOTE_LIQUIDITY=100000` control bootstrap reserves.
- `OMNIPAIR_V2_FORCE_SEED=1` adds more bootstrap liquidity to an existing market.
- `OMNIPAIR_V2_SMOKE_OPEN_HEDGE=0` skips the default smoke-test hLP open.
- `OMNIPAIR_V2_SMOKE_HEDGE_AMOUNT=10` controls the smoke-test base hLP open amount.
- `OMNIPAIR_V2_SMOKE_SWAP=0` fetches state without sending the smoke swap.
