# Verifiable Builds

This directory contains verifiable builds of the Omnipair program.

## Files

- `omnipair.so` - The compiled Solana program binary
- `omnipair.json` - The program IDL (Interface Definition Language)

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

# Verify against mainnet
solana-verify verify-from-repo \
  --remote -um \
  --program-id <PROGRAM_ID> \
  https://github.com/omnipair/omnipair-rs \
  --library-name omnipair
```

Or use the `Verify Build` GitHub Action workflow.

## Regenerating

Builds are automatically regenerated on push to `main` via the `generate-verifiable-builds` workflow.

To manually regenerate:
1. Go to Actions → "generate-verifiable-builds"
2. Click "Run workflow"
