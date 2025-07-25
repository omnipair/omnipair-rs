# omnipair-rs


### Local and Devnet flow for get up and running
1. Build and Deploy the Program:
   ```bash
   anchor keys sync
   anchor build -- --features "development"
   anchor deploy
   ```

2. Create Development Token Pair (with the new deployed program id as the mint authority):
   ```bash
   yarn deploy-tokens
   ```
   Update `.env` with the new token mint addresses:
   ```
   TOKEN0_MINT=<new_token0_mint_address>
   TOKEN1_MINT=<new_token1_mint_address>
   ```

3. Initialize the Pair:
   ```bash
   yarn initialize
   ```

4. Mint Test Tokens:
   ```bash
   yarn faucet-mint
   ```
   Update `.env` with your token account addresses:
   ```
   DEPLOYER_TOKEN0_ACCOUNT=<your_token0_account>
   DEPLOYER_TOKEN1_ACCOUNT=<your_token1_account>
   ```

5. Bootstrap Liquidity:
   ```bash
   yarn bootstrap
   ```

6. Pubish IDL
```
 anchor idl init --filepath target/idl/omnipair.json [program.id] --provider.cluster devnet
 ```

After completing these steps, you can:
- Add and remove liquidity
- Add and remove collateral
- Borrow and repay loans


For production run: 
   ```bash
   anchor keys sync
   anchor build --verifiable -- --features "production"
   anchor deploy --verifiable
   anchor idl init --filepath target/idl/omnipair.json <program-id> --provider.cluster mainnet
   anchor verify -p omnipair <program-id>
   ```