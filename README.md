# omnipair-rs

## Network Configuration

This project supports both devnet and mainnet deployments using Cargo features and environment variables.

### Environment Variables

You can set these environment variables to configure the deployment:

- `ANCHOR_CLUSTER`: Network cluster (devnet/mainnet)
- `ANCHOR_WALLET`: Path to wallet keypair file
- `ANCHOR_REGISTRY_URL`: RPC endpoint URL

### Quick Start

**For Devnet (default):**
```bash
anchor keys sync
anchor build -- --features "development"
anchor deploy
```

**For Mainnet:**
```bash
# Set environment variables
export ANCHOR_CLUSTER=mainnet
export ANCHOR_WALLET=mainnet-keypair.json
export ANCHOR_REGISTRY_URL=https://api.mainnet-beta.solana.com

# Build and deploy
anchor keys sync
anchor build -- --features "production"
```

### Development Flow

1. Create Development Token Pair (with the new deployed program id as the mint authority):
   ```bash
   yarn deploy-tokens
   ```
   Update `.env` with the new token mint addresses:
   ```
   TOKEN0_MINT=<new_token0_mint_address>
   TOKEN1_MINT=<new_token1_mint_address>
   ```

2. Initialize Futarchy Authority and Pair Config:
   ```bash
   yarn init-futarchy
   ```

3. Initialize the Pair:
   ```bash
   yarn initialize
   ```

4. Mint Test Tokens:
   ```bash
   yarn faucet-mint
   ```

5. Bootstrap Liquidity:
   ```bash
   yarn bootstrap
   ```

6. Publish IDL
```bash
# For devnet
anchor idl init --filepath target/idl/omnipair.json [program.id]

# For mainnet
ANCHOR_CLUSTER=mainnet anchor idl init --filepath target/idl/omnipair.json [program.id]
```

After completing these steps, you can:
- Add and remove liquidity
- Add and remove collateral
- Borrow and repay loans


### Production Deployment

For production deployment with verification:

```bash
# Set environment variables
export ANCHOR_CLUSTER=mainnet
export ANCHOR_WALLET=mainnet-keypair.json
export ANCHOR_REGISTRY_URL=https://api.mainnet-beta.solana.com

# Build, deploy, and verify
anchor keys sync
anchor build --verifiable -- --features "production"
anchor deploy --verifiable
anchor idl init --filepath target/idl/omnipair.json <program-id>
anchor verify -p omnipair <program-id>
```

### Program Upgrade

To upgrade an existing deployed program:

```bash
# Set environment variables for mainnet
export ANCHOR_CLUSTER=mainnet
export ANCHOR_WALLET=mainnet-keypair.json
export ANCHOR_REGISTRY_URL=https://mainnet.helius-rpc.com/?api-key={YOUR_API_KEY}

# Build the new program
anchor build -- --features "production"

# Upgrade the program
anchor upgrade --provider.cluster https://mainnet.helius-rpc.com/?api-key={YOUR_API_KEY} \
   --program-id 3tJrAXnjofAw8oskbMaSo9oMAYuzdBgVbW3TvQLdMEBd \
   ./target/deploy/omnipair.so

// for devnet
anchor upgrade --provider.cluster https://devnet.helius-rpc.com/?api-key=66a4060b-2453-49cd-bf7a-fa03546c97ec --program-id 6boPPughAjq1PeoEicamfirB9SYjF8bBCSCeUvKJeZMj ./target/deploy/omnipair.so
```

**Note:** Replace `{YOUR_API_KEY}` with your actual Helius API key and `3tJrAXnjofAw8oskbMaSo9oMAYuzdBgVbW3TvQLdMEBd` with your actual program ID.