# Omnipair Scripts

This directory contains TypeScript scripts for interacting with the Omnipair protocol.

## Updated Scripts for New Pair Config Structure

The scripts have been updated to work with the new pair config structure that includes futarchy authority and pair config PDAs with proper seeds and deployment parameters.

### New Scripts

#### `init_futarchy_and_config.ts`
Initializes the futarchy authority and pair config with the proper PDAs and deployment parameters.

**Usage:**
```bash
npm run ts-node scripts/init_futarchy_and_config.ts
```

This script:
1. Creates the futarchy authority PDA with seed `futarchy_authority`
2. Creates the pair config PDA with seed `gamm_pair_config` + nonce
3. Initializes the pair config with futarchy fees and founder fees

**Output:**
- Futarchy Authority PDA address
- Pair Config PDA address
- Pair Config Nonce (for reference)

### Updated Scripts

#### `initialize_pair.ts`
Updated to work with an existing pair config PDA instead of creating a new one.

**Environment Variables Required:**
- `TOKEN0_MINT`: Address of the first token mint
- `TOKEN1_MINT`: Address of the second token mint
- `PAIR_CONFIG_PDA`: Address of the existing pair config PDA (from `init_futarchy_and_config.ts`)

**Usage:**
```bash
PAIR_CONFIG_PDA=<pair_config_pda_address> npm run ts-node scripts/initialize_pair.ts
```

This script:
1. Uses the provided pair config PDA
2. Creates a new rate model account
3. Initializes the pair with the existing pair config

### Other Updated Scripts

The following scripts have been updated to correctly access the rate model from the pair account instead of the pair config account:

- `bootstrap_liquidity.ts`
- `add_collateral.ts`
- `borrow.ts`
- `add_liquidity.ts`
- `swap.ts`
- `remove_collateral.ts`
- `liquidate.ts`
- `remove_liquidity.ts`
- `repay.ts`

## Deployment Workflow

1. **Initialize Futarchy and Pair Config:**
   ```bash
   npm run ts-node scripts/init_futarchy_and_config.ts
   ```

2. **Set the Pair Config PDA as an environment variable:**
   ```bash
   export PAIR_CONFIG_PDA=<pair_config_pda_address>
   ```

3. **Initialize Pair:**
   ```bash
   npm run ts-node scripts/initialize_pair.ts
   ```

4. **Bootstrap Liquidity:**
   ```bash
   npm run ts-node scripts/bootstrap_liquidity.ts
   ```

## Fee Structure

The new pair config structure includes:

- **Futarchy Fee**: 0.5% (50 bps) - Fee collected by the futarchy authority
- **Founder Fee**: 0.3% (30 bps) - Fee collected by the founder
- **Swap Fee**: 0.3% (30 bps) - Fee collected by the pool
- **Pool Deployer Fee**: 0.1% (10 bps) - Fee collected by the pool deployer

## PDA Seeds

- **Futarchy Authority**: `futarchy_authority`
- **Pair Config**: `gamm_pair_config` + nonce (as bytes)
- **Pair**: `gamm_pair` + token0 + token1
- **LP Mint**: `gamm_lp_mint` + pair
- **User Position**: `gamm_position` + pair + user

## Account Structure

### Pair Config Account
- `futarchy_fee_bps`: u16 - Futarchy fee in basis points
- `founder_fee_bps`: u16 - Founder fee in basis points  
- `nonce`: u64 - Unique identifier for the pair config

### Pair Account
- `token0`: Pubkey - First token mint
- `token1`: Pubkey - Second token mint
- `config`: Pubkey - Reference to pair config account
- `rate_model`: Pubkey - Reference to rate model account
- `swap_fee_bps`: u16 - Swap fee in basis points
- `half_life`: u64 - EMA half-life in seconds
- `pool_deployer_fee_bps`: u16 - Pool deployer fee in basis points
- Plus other pair state fields (reserves, debt, etc.)

## Notes

- The rate model is now a separate account referenced in the pair account
- The pair config contains only futarchy and founder fees
- All scripts now correctly access the rate model from `pairAccount.rateModel`
- The swap fee and other pair parameters are stored in the pair account itself
- The `nonce` parameter in pair config initialization must be a BN (Big Number)
- Account names in TypeScript use snake_case to match Rust struct fields
