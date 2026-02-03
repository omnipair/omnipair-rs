# omnipair-rs

**Omnipair** is a next-generation DeFi primitive on Solana that unifies liquidity provision and lending into a single capital-efficient protocol.

## Overview

Omnipair combines an Automated Market Maker (AMM) with an integrated lending market, allowing liquidity providers to earn both swap fees and lending interest on their deposited assets. Borrowers can use one side of the pair as collateral to borrow the other, with dynamic collateral factors that adjust based on real-time price movements.

### Key Features

- **Unified Liquidity** - LP deposits serve as both AMM reserves and lending supply, maximizing capital efficiency
- **Dynamic Collateral Factors** - Collateral ratios automatically adjust based on price EMA (Exponential Moving Average) to protect against volatility
- **Directional EMA Oracle** - Built-in price oracle using asymmetric EMA that responds instantly to price drops but smooths price increases
- **Flash Loans** - Uncollateralized loans within a single transaction (0.05% fee)
- **Interest Rate Model** - Adaptive rates based on utilization with configurable target ranges
- **Liquidation Engine** - Partial liquidations with 3% penalty (0.5% to liquidator, 2.5% to LPs)

### How It Works

```
┌─────────────────────────────────────────────────────────────────┐
│                         OMNIPAIR                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Liquidity Providers              Traders                      │
│   ┌───────────────────┐              ┌─────────────────┐        │
│   │ Deposit Token A   │              │ Swap A ↔ B      │        │
│   │ Deposit Token B   │              │ (pay swap fee)  │        │
│   │ Receive LP Tokens │              └─────────────────┘        │
│   └───────────────────┘                                         │
│           │                                                     │
│           ▼                                                     │
│   ┌─────────────────────────────────────────────────┐           │
│   │              Unified Reserve Pool               │           │
│   │  ┌──────────────────┐  ┌──────────────────┐     │           │
│   │  │ Token A Reserve  │  │ Token B Reserve  │     │           │
│   │  │ (Cash + Debt)    │  │ (Cash + Debt)    │     │           │
│   │  └──────────────────┘  └──────────────────┘     │           │
│   └─────────────────────────────────────────────────┘           │
│           │                                                     │
│           ▼                                                     │
│   Borrowers                                                     │
│   ┌─────────────────┐                                           │
│   │ Deposit Token A │  →  Borrow Token B                        │
│   │ as Collateral   │  ←  (pay interest)                        │
│   └─────────────────┘                                           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Protocol Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| Swap Fee | Configurable | Per-pair swap fee in basis points |
| Flash Loan Fee | 0.05% | Fee for uncollateralized flash loans |
| Max Collateral Factor | 85% | Maximum LTV before liquidation risk |
| LTV Buffer | 5% | Gap between borrow limit and liquidation |
| Liquidation Penalty | 3% | Total penalty on liquidated collateral |
| Liquidation Incentive | 0.5% | Reward for liquidators |
| LP Withdrawal Fee | 1% | Fee to remaining LPs on withdrawal |

### Audits

Omnipair has been audited by:
- **Offside Labs**
- **Ackee**

See [security policy](https://omnipair.fi/security) for details.

---

## Program Addresses

| Network | Program ID |
|---------|------------|
| Mainnet | `omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE` |
| Devnet | `omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE` |

## Quick Start

### Local Development

```bash
# Install dependencies
yarn install

# Build for devnet
anchor build

# Run tests
anchor test
```

### Environment Variables

Create a `.env` file based on `.env.example`:

```bash
cp .env.example .env
```

Key variables:
- `ANCHOR_CLUSTER`: Network cluster (devnet/mainnet)
- `ANCHOR_WALLET`: Path to wallet keypair file
- `TOKEN0_MINT` / `TOKEN1_MINT`: Token mint addresses for the pair

## Development Flow

1. **Deploy Test Tokens** (devnet only):
   ```bash
   yarn deploy-tokens
   ```
   Update `.env` with the new token mint addresses.

2. **Initialize Futarchy Authority**:
   ```bash
   yarn init-futarchy
   ```

3. **Mint Test Tokens**:
   ```bash
   yarn faucet-mint
   ```

4. **Initialize the Pair**:
   ```bash
   yarn initialize
   ```

5. **Publish IDL**:
   ```bash
   anchor idl init --filepath target/idl/omnipair.json <PROGRAM_ID>
   ```

---

## CI/CD & Release Workflow

This project uses automated CI/CD with GitHub Actions for releases and program upgrades.

### Release Flow Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        AUTOMATIC (PR Merge)                         │
├─────────────────────────────────────────────────────────────────────┤
│  1. Version Bump      →  Based on conventional commits              │
│  2. Verifiable Build  →  anchor build --verifiable --features prod  │
│  3. Create Release    →  GitHub release with .so and IDL artifacts  │
└─────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    MANUAL: Deploy Buffer (~8 SOL)                   │
├─────────────────────────────────────────────────────────────────────┤
│  4. Download from Release  →  Gets omnipair.so from GitHub          │
│  5. Deploy Buffer          →  solana program write-buffer           │
│  6. Transfer to Squads     →  Buffer authority → multisig           │
└─────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      MANUAL: Squads Approval                        │
├─────────────────────────────────────────────────────────────────────┤
│  Team signs upgrade transaction on Squads UI                        │
│  https://app.squads.so/squads/<MULTISIG>/developer/programs/<ID>    │
└─────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│                   MANUAL: Verify & Publish Packages                 │
├─────────────────────────────────────────────────────────────────────┤
│  7. Verify Release   →  solana-verify + OtterSec submission         │
│  8. Publish npm      →  @omnipair/program-interface                 │
│  9. Publish crate    →  omnipair-decoder on crates.io               │
└─────────────────────────────────────────────────────────────────────┘
```

### Conventional Commits

Version bumps are automatic based on commit messages:

| Commit Prefix | Version Bump | Example |
|---------------|--------------|---------|
| `fix:` | PATCH (0.0.X) | `fix: correct swap calculation` |
| `feat:` | MINOR (0.X.0) | `feat: add flash loan support` |
| `feat!:` or `BREAKING CHANGE:` | MAJOR (X.0.0) | `feat!: new account structure` |
| `chore:`, `docs:`, `test:` | No release | `chore: update dependencies` |

### GitHub Actions Workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `release-build.yaml` | PR merge / Manual | Build, release, deploy, verify, publish |
| `anchor-buffer.yaml` | Manual | Standalone buffer deployment (edge cases) |
| `verify-build.yaml` | Manual | Verify on-chain program against source |

### Manual Workflow Triggers

All manual triggers: **Actions → release-build → Run workflow**

| Input | Purpose |
|-------|---------|
| `version` | Explicit version (e.g., `1.0.0`) |
| `bump_type` | Auto/patch/minor/major bump |
| `deploy_buffer` ✅ | Deploy buffer to Solana mainnet (~8 SOL) |
| `verify_only` ✅ | Only verify on-chain program |
| `publish_packages` ✅ | Verify + publish npm/crates.io |

**Typical Upgrade Flow:**
```
1. Merge PR           →  Auto creates release v0.4.0
2. deploy_buffer ✅   →  Deploys buffer, transfers to Squads
3. Team signs         →  Approve on Squads UI
4. publish_packages ✅ →  Verify + publish packages
```

**Deploy Buffer Only** (edge cases):
```
Actions → Manual Buffer Deploy → Run workflow
  ├── source: release (from GitHub release)
  └── release_tag: v0.4.0 (optional, defaults to latest)
```

---

## Verifiable Builds

All releases are built using Anchor's verifiable build system for reproducibility.

### Build Locally

```bash
# Set environment variables for security.txt
export GIT_REV=$(git rev-parse HEAD)
export GIT_RELEASE=$(git describe --tags --abbrev=0 2>/dev/null || echo "dev")

# Build verifiable
anchor build --verifiable -p omnipair \
  -e GIT_REV=$GIT_REV \
  -e GIT_RELEASE=$GIT_RELEASE \
  -- --features "production"
```

### Verify On-Chain Program

```bash
# Install solana-verify
cargo install solana-verify

# Verify from repository
solana-verify verify-from-repo \
  --remote \
  --program-id omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE \
  https://github.com/omnipair/omnipair-rs \
  --commit-hash <COMMIT_SHA> \
  --library-name omnipair \
  --bpf-flag "features=production"
```

### Submit to OtterSec Registry

```bash
solana-verify remote submit-job \
  --program-id omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE \
  https://github.com/omnipair/omnipair-rs \
  --commit-hash <COMMIT_SHA> \
  --library-name omnipair \
  --bpf-flag "features=production"
```

---

## Program Upgrades (Multisig)

The program upgrade authority is a Squads multisig. Upgrades require team approval.

### Automated Flow (via CI)

1. Merge PR to `main` with `feat:` or `fix:` commit
2. CI builds verifiable binary and creates GitHub release
3. **Manual:** Run workflow with `deploy_buffer` ✅ to deploy buffer
4. **Manual:** Team signs upgrade transaction on Squads UI
5. **Manual:** Run workflow with `publish_packages` ✅ to verify + publish

### Manual Upgrade Flow

If you need to upgrade manually:

```bash
# 1. Build verifiable binary
export GIT_REV=$(git rev-parse HEAD)
export GIT_RELEASE=$(git describe --tags)
anchor build --verifiable -p omnipair \
  -e GIT_REV=$GIT_REV \
  -e GIT_RELEASE=$GIT_RELEASE \
  -- --features "production"

# 2. Deploy buffer
solana program write-buffer \
  --keypair deployer-keypair.json \
  target/verifiable/omnipair.so \
  -u mainnet-beta

# 3. Transfer authority to Squads vault
solana program set-buffer-authority <BUFFER_ADDRESS> \
  --new-buffer-authority <SQUADS_VAULT_ADDRESS> \
  --keypair deployer-keypair.json \
  -u mainnet-beta

# 4. Create upgrade proposal on Squads UI
# https://app.squads.so/squads/<MULTISIG_ADDRESS>/developer/programs/<PROGRAM_ID>

# 5. Team signs and executes

# 6. Verify
solana-verify verify-from-repo \
  --remote \
  -um \
  --program-id <PROGRAM_ID> \
  https://github.com/omnipair/omnipair-rs \
  --library-name omnipair \
  --bpf-flag "features=production"
```

### Extend Program Size (if needed)

If the new binary is larger than allocated space:

```bash
# Check current size
solana program show omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE

# Extend (requires upgrade authority - do via Squads)
solana program extend omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE <ADDITIONAL_BYTES>
```

---

## GitHub Repository Configuration

### Required Secrets

| Secret | Description |
|--------|-------------|
| `DEPLOYER_KEYPAIR` | JSON array of funded deployer wallet (~8 SOL for buffer) |
| `NPM_TOKEN` | npm access token for publishing |
| `CRATES_IO_TOKEN` | crates.io API token for publishing decoder |
| `GH_PAT` | GitHub PAT with repo write access (for version bump commits) |

### Required Variables

| Variable | Description |
|----------|-------------|
| `SQUADS_MULTISIG_ADDRESS` | Squads multisig address |
| `SQUADS_VAULT_ADDRESS` | Squads vault PDA (buffer authority recipient) |
| `MAINNET_RPC_URL` | (Optional) Custom RPC URL |

> **Note:** `PROGRAM_ID` is automatically extracted from `programs/omnipair/src/lib.rs` (`declare_id!` macro).

### Finding Squads Vault Address

```typescript
import { getVaultPda } from "@sqds/multisig";

const [vault] = getVaultPda({
  multisigPda: new PublicKey("YOUR_MULTISIG_ADDRESS"),
  index: 0,
});
console.log("Vault:", vault.toBase58());
```

---

## Project Structure

```
omnipair-rs/
├── programs/
│   └── omnipair/           # Main program
│       ├── src/
│       │   ├── lib.rs
│       │   ├── instructions/
│       │   ├── state/
│       │   └── utils/
│       └── Cargo.toml
├── scripts/                # TypeScript helper scripts
├── tests/                  # Integration tests
├── packages/
│   └── program-interface/  # npm package with IDL
└── .github/workflows/      # CI/CD workflows
```

## License

See [LICENSE](./LICENSE) for details.