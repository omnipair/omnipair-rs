# omnipair-rs

**Omnipair** is a next-generation DeFi primitive on Solana that unifies liquidity provision and lending into a single capital-efficient protocol.

## Overview

Omnipair [GAMM](https://docs.omnipair.fi/technical-breakdown/generalized-automated-market-maker) (Generalized Automated Market Maker) combines a UniV2-style CPMM (constant-product market maker) with an integrated lending market, allowing liquidity providers to earn both swap fees and lending interest on their deposited assets. Borrowers can use one side of the pair as collateral to borrow the other. Price uses a built-in **symmetric EMA** (smooths both up and down over a half-life); **dynamic LTV and collateral factors** use a **double-EMA** (symmetric + directional), taking the more conservative of the two to protect against volatility and front-running. 

Lending is **isolated** per pair (each pool’s risk is contained), and **bad debt is socialized on LPs** (insolvent positions are written off against the pool so the protocol avoids bank runs; LPs bear the loss via the constant-product curve). The protocol is **oracless** (no external oracle dependency) and **permissionless** (anyone can create pairs, add liquidity, borrow, or liquidate).

Beyond the AMM invariant *xy* = *k*, Omnipair enforces a **lending solvency invariant**: virtual reserves = cash + debt (`R_virtual = R_cash + R_debt`) with `R_cash ≥ 0`, and every state change obeys `ΔR_virtual = ΔR_cash + ΔR_debt`. See the [Docs](https://docs.omnipair.fi/technical-breakdown/overview) for reserve types, Impact-aware collateral factor and how liquidation works.

## Program Generations

This repository now contains two Omnipair program generations:

- `programs/omnipair`: legacy V1 GAMM pair program. Existing pair accounts, instruction names, and integrations remain compatible.
- `programs/omnipair-v2`: standalone V2 market architecture program with market accounts, claim-token (`omLP`) liquidity, hedge-token (`h-omLP`) wrappers, fixed debt, market health, insurance, and V2-specific events/IDL.

V2 review and integration entry points:

- [V2_PR_REVIEW_GUIDE.md](V2_PR_REVIEW_GUIDE.md): recommended review order,
  commit grouping, verification gates, and production gates for the V2 branch.
- [programs/omnipair-v2/README.md](programs/omnipair-v2/README.md): architecture, invariants, integrator handoff, and verification gates.
- [programs/omnipair-v2/RELEASE_CHECKLIST.md](programs/omnipair-v2/RELEASE_CHECKLIST.md): security, artifact, deployment, and post-deploy checklist.
- [programs/omnipair-v2/SIGNOFF_CHECKLIST.md](programs/omnipair-v2/SIGNOFF_CHECKLIST.md): owner signoff register for security, app, SDK, indexing, analytics, aggregators, and deployment.
- [packages/program-interface/README.md](packages/program-interface/README.md): V1/V2 TypeScript IDL, type, and PDA helper usage.
- [decoders/omnipair-decoder/README.md](decoders/omnipair-decoder/README.md): Carbon decoder usage for legacy V1 and standalone V2.
- [tests/README.md](tests/README.md): LiteSVM smoke coverage and V2 test flow notes.

### Legacy V1 Key Features

- **Unified Liquidity** - LP deposits serve as both AMM reserves and lending supply, maximizing capital efficiency
- **Isolated Lending** - Risk is isolated per pair; each pool’s borrows and collateral are independent of other pairs
- **Bad debt socialized on LPs** - Insolvent positions are written off against the pool; LPs absorb the loss (via the AMM curve) to protect the protocol from bank runs
- **Lending EMA price** - Built-in price uses a **symmetric EMA** that smooths both up and down movements over a configurable half-life
- **Dynamic LTV / collateral factors: double-EMA** - Two EMAs (**symmetric** and **directional**) feed dynamic LTV and collateral ratios; the more conservative of the two is used for borrow and liquidation limits to protect LPs and borrowers
- **Flash Loans** - Uncollateralized loans within a single transaction (0.05% fee)
- **Interest Rate Model** - Adaptive rates based on utilization with configurable target ranges
- **Liquidation Engine** - Partial liquidations with 3% penalty (0.5% to liquidator, 2.5% to LPs)

### V2 Key Changes

- **Standalone market program** - V2 has its own program ID, IDL, accounts, events, and SDK helpers.
- **Fixed-principal claim tokens** - LP principal is represented by 1:1 `omLP` claim tokens; fees do not rebase or compound into claim-token exchange rates.
- **Matched staking for fees** - Fee rights require staking claim tokens with matched junior buffer shares.
- **Recognized-collateral health** - Borrow health uses debt-bearing recognized collateral, not idle collateral balances.
- **Cached EMA risk books** - Risk checks roll EMA values from cached observations to avoid same-instruction spot manipulation.
- **Insurance and hedge overlays** - V2 adds insurance reserves and `h-omLP` claim-token wrappers without giving hedge tokens staking rights.

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

| Program | Network | Program ID |
|---------|---------|------------|
| Omnipair V1 | Mainnet | `omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE` |
| Omnipair V1 | Devnet | `omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE` |
| Omnipair V2 | Mainnet | `358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv` |
| Omnipair V2 | Devnet | `358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv` |

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
│  2. Verifiable Build  →  production-feature Anchor build             │
│  3. Create Release    →  GitHub release with .so and IDL artifacts  │
└─────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    MANUAL: Deploy Buffer (~8 SOL)                   │
├─────────────────────────────────────────────────────────────────────┤
│  4. Download from Release  →  Gets omnipair*.so from GitHub         │
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

### Manual Workflow Triggers

Release, deploy, verify, and publish triggers: **Actions → release-build → Run workflow**

| Input | Purpose |
|-------|---------|
| `version` | Explicit version (e.g., `1.0.0`) |
| `bump_type` | Auto/patch/minor/major bump |
| `deploy_buffer` ✅ | Deploy buffer to Solana mainnet (~8 SOL) |
| `verify_only` ✅ | Only verify on-chain program |
| `publish_packages` ✅ | Verify + publish npm/crates.io |
| `program` | Select V1 or V2 for manual deploy/verify jobs |

Standalone buffer redeploys use **Actions → Manual Buffer Deploy → Run workflow**.

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
  ├── program: v1 or v2
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

# Build V2 verifiable
anchor build --verifiable -p omnipair-v2 \
  -e GIT_REV=$GIT_REV \
  -e GIT_RELEASE=$GIT_RELEASE \
  -- --features "production"
```

### Verify On-Chain Program

```bash
# Install solana-verify
cargo install solana-verify

COMMIT_SHA=<COMMIT_SHA>
RELEASE_TAG=<RELEASE_TAG>

# Verify from repository
solana-verify verify-from-repo \
  --skip-prompt \
  --base-image solanafoundation/anchor:v0.31.1 \
  --program-id omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE \
  https://github.com/omnipair/omnipair-rs \
  --commit-hash "$COMMIT_SHA" \
  --library-name omnipair \
  -u mainnet-beta \
  -- --features production \
     --config "env.GIT_REV=\"$COMMIT_SHA\"" \
     --config "env.GIT_RELEASE=\"$RELEASE_TAG\""

# Verify V2 from repository
solana-verify verify-from-repo \
  --skip-prompt \
  --base-image solanafoundation/anchor:v0.31.1 \
  --program-id 358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv \
  https://github.com/omnipair/omnipair-rs \
  --commit-hash "$COMMIT_SHA" \
  --library-name omnipair_v2 \
  -u mainnet-beta \
  -- --features production \
     --config "env.GIT_REV=\"$COMMIT_SHA\"" \
     --config "env.GIT_RELEASE=\"$RELEASE_TAG\""
```

### Submit to OtterSec Registry

```bash
SQUADS_VAULT=<SQUADS_VAULT_ADDRESS>

# Export the verification PDA transaction, submit it through Squads, then:
solana-verify remote submit-job \
  --program-id omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE \
  --uploader "$SQUADS_VAULT"

# Submit V2 to OtterSec Registry
solana-verify remote submit-job \
  --program-id 358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv \
  --uploader "$SQUADS_VAULT"
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
# 1. Pick program generation
# V1:
PROGRAM_CRATE=omnipair
PROGRAM_LIBRARY=omnipair
PROGRAM_SO=omnipair.so
PROGRAM_ID=omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE

# V2:
# PROGRAM_CRATE=omnipair-v2
# PROGRAM_LIBRARY=omnipair_v2
# PROGRAM_SO=omnipair_v2.so
# PROGRAM_ID=358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv

# 2. Build verifiable binary
export GIT_REV=$(git rev-parse HEAD)
export GIT_RELEASE=$(git describe --tags --abbrev=0 2>/dev/null || echo "dev")
anchor build --verifiable -p $PROGRAM_CRATE \
  -e GIT_REV=$GIT_REV \
  -e GIT_RELEASE=$GIT_RELEASE \
  -- --features "production"

# 3. Deploy buffer
solana program write-buffer \
  --keypair deployer-keypair.json \
  target/verifiable/$PROGRAM_SO \
  -u mainnet-beta

# 4. Transfer authority to Squads vault
solana program set-buffer-authority <BUFFER_ADDRESS> \
  --new-buffer-authority <SQUADS_VAULT_ADDRESS> \
  --keypair deployer-keypair.json \
  -u mainnet-beta

# 5. Create upgrade proposal on Squads UI
# https://app.squads.so/squads/<MULTISIG_ADDRESS>/developer/programs/<PROGRAM_ID>

# 6. Team signs and executes

# 7. Verify
COMMIT_SHA=$(git rev-parse HEAD)
RELEASE_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "dev")
solana-verify verify-from-repo \
  --skip-prompt \
  --base-image solanafoundation/anchor:v0.31.1 \
  --program-id $PROGRAM_ID \
  https://github.com/omnipair/omnipair-rs \
  --commit-hash "$COMMIT_SHA" \
  --library-name $PROGRAM_LIBRARY \
  -u mainnet-beta \
  -- --features production \
     --config "env.GIT_REV=\"$COMMIT_SHA\"" \
     --config "env.GIT_RELEASE=\"$RELEASE_TAG\""
```

### Extend Program Size (if needed)

If the new binary is larger than allocated space:

```bash
# Check current size
solana program show <PROGRAM_ID>

# Extend (requires upgrade authority - do via Squads)
solana program extend <PROGRAM_ID> <ADDITIONAL_BYTES>
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

> **Note:** CI extracts the selected program ID from the matching `declare_id!` macro: `programs/omnipair/src/lib.rs` for V1 and `programs/omnipair-v2/src/lib.rs` for V2.

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
│   ├── omnipair/           # Legacy V1 pair program
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── instructions/
│   │   │   ├── state/
│   │   │   └── utils/
│   │   └── Cargo.toml
│   └── omnipair-v2/        # Standalone V2 market program
│       ├── src/
│       └── Cargo.toml
├── scripts/                # TypeScript helper scripts
├── tests/                  # Integration tests
├── packages/
│   └── program-interface/  # npm package with IDL
└── .github/workflows/      # CI/CD workflows
```

## License

See [LICENSE](./LICENSE) for details.
