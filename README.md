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
| Mainnet | `omniSVEL3cY36TYhunvJC6vBXxbJrqrn7JhDrXUTerb` |
| Devnet | `omniSVEL3cY36TYhunvJC6vBXxbJrqrn7JhDrXUTerb` |

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
PR Merge to main
       │
       ▼
┌──────────────────────┐
│ 1. Version Bump      │  Automatic based on conventional commits
│ 2. Verifiable Build  │  anchor build --verifiable --features production
│ 3. Create Release    │  GitHub release with artifacts
│ 4. Deploy Buffer     │  solana program write-buffer
│ 5. Transfer to Squads│  Buffer authority → multisig vault
│ 6. Publish npm       │  @omnipair/program-interface
└──────────────────────┘
       │
       ▼
┌──────────────────────┐
│ MANUAL: Squads Sign  │  Team signs upgrade transaction
└──────────────────────┘
       │
       ▼
┌──────────────────────┐
│ 7. Verify Release    │  solana-verify verify-from-repo
└──────────────────────┘
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
| `release-build.yaml` | PR merge to main | Full release: build, buffer, npm |
| `anchor-buffer.yaml` | Manual | Standalone buffer deployment |
| `generate-verifiable-builds.yaml` | Push to main | Build artifacts without release |

### Manual Workflow Triggers

**Create a Release** (if automatic didn't trigger):
```
Actions → release-build → Run workflow
  └── bump_type: patch/minor/major
```

**Verify After Squads Execution**:
```
Actions → release-build → Run workflow
  └── verify_only: ✅ (checked)
```

**Deploy Buffer Manually** (edge cases):
```
Actions → Manual Buffer Deploy → Run workflow
  ├── network: mainnet-beta
  ├── source: release
  └── release_tag: v0.1.0
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
  --program-id omniSVEL3cY36TYhunvJC6vBXxbJrqrn7JhDrXUTerb \
  https://github.com/omnipair/omnipair-rs \
  --commit-hash <COMMIT_SHA> \
  --library-name omnipair \
  --bpf-flag "features=production"
```

### Submit to OtterSec Registry

```bash
solana-verify remote submit-job \
  --program-id omniSVEL3cY36TYhunvJC6vBXxbJrqrn7JhDrXUTerb \
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
2. CI builds verifiable binary and deploys buffer
3. CI transfers buffer authority to Squads vault
4. Team signs upgrade transaction on Squads UI
5. Run verification workflow after execution

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
solana-verify verify-from-repo ...
```

### Extend Program Size (if needed)

If the new binary is larger than allocated space:

```bash
# Check current size
solana program show omniSVEL3cY36TYhunvJC6vBXxbJrqrn7JhDrXUTerb

# Extend (requires upgrade authority - do via Squads)
solana program extend omniSVEL3cY36TYhunvJC6vBXxbJrqrn7JhDrXUTerb <ADDITIONAL_BYTES>
```

---

## GitHub Repository Configuration

### Required Secrets

| Secret | Description |
|--------|-------------|
| `DEPLOYER_KEYPAIR` | JSON array of funded deployer wallet |
| `NPM_TOKEN` | npm access token for publishing |

### Required Variables

| Variable | Description |
|----------|-------------|
| `PROGRAM_ID` | `omniSVEL3cY36TYhunvJC6vBXxbJrqrn7JhDrXUTerb` |
| `SQUADS_MULTISIG_ADDRESS` | Squads multisig address |
| `SQUADS_VAULT_ADDRESS` | Squads vault PDA (buffer authority recipient) |
| `MAINNET_RPC_URL` | (Optional) Custom RPC URL |

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