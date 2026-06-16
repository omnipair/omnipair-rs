# Verifiable Builds

This directory contains verifiable builds of the Omnipair programs.

## Files

- `omnipair.so` - The compiled legacy V1 Solana program binary
- `omnipair.json` - The legacy V1 program IDL (Interface Definition Language)
- `omnipair_v2.so` - The compiled V2 market program binary, when generated
- `omnipair_v2.json` - The V2 market program IDL, when generated

## Build Configuration

These builds are generated with:
- Anchor: 0.31.1
- Solana: 1.18.18
- Features: `production`

## Verification

To verify a deployed program matches this build:

```bash
# Install solana-verify
cargo install solana-verify

# Verify V1 against mainnet
solana-verify verify-from-repo \
  --remote -um \
  --program-id omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE \
  https://github.com/omnipair/omnipair-rs \
  --library-name omnipair

# Verify V2 against mainnet
solana-verify verify-from-repo \
  --remote -um \
  --program-id 358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv \
  https://github.com/omnipair/omnipair-rs \
  --library-name omnipair_v2
```

Or use the `Verify Build` GitHub Action workflow.

## Regenerating

Builds are automatically regenerated on push to `main` via the `generate-verifiable-builds` workflow.

To manually regenerate:
1. Go to Actions → "generate-verifiable-builds"
2. Click "Run workflow"
