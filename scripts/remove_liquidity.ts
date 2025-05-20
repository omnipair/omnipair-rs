import { 
    PublicKey, 
    SystemProgram,
} from '@solana/web3.js';
import { 
    TOKEN_PROGRAM_ID, 
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
    getAssociatedTokenAddress,
    getAccount
} from '@solana/spl-token';
import { Program } from '@coral-xyz/anchor';
import idl from '../target/idl/omnipair.json' with { type: "json" };
import type { Omnipair } from '../target/types/omnipair';
import BN from 'bn.js';
import * as anchor from '@coral-xyz/anchor';
import * as dotenv from 'dotenv';

// Load environment variables
dotenv.config();

// Replace these with your actual values
const TOKEN0_MINT = new PublicKey(process.env.TOKEN0_MINT || '');
const TOKEN1_MINT = new PublicKey(process.env.TOKEN1_MINT || '');

// Token accounts that already exist
const DEPLOYER_TOKEN0_ACCOUNT = new PublicKey(process.env.DEPLOYER_TOKEN0_ACCOUNT || '');
const DEPLOYER_TOKEN1_ACCOUNT = new PublicKey(process.env.DEPLOYER_TOKEN1_ACCOUNT || '');

async function main() {
    console.log('Starting liquidity removal...');
    
    // Setup connection and provider using Anchor configuration
    const provider = anchor.AnchorProvider.env();
    const program = new Program<Omnipair>(idl, provider);
    const DEPLOYER_KEYPAIR = provider.wallet.payer;
    
    if(!DEPLOYER_KEYPAIR) {
        throw new Error('Deployer keypair not found');
    }

    // Set proper commitment levels
    provider.opts.commitment = 'confirmed';
    provider.opts.preflightCommitment = 'confirmed';
    provider.opts.skipPreflight = false;

    // Log all addresses
    console.log('Network:', provider.connection.rpcEndpoint);
    console.log('Program ID:', program.programId.toBase58());
    console.log('Deployer address:', provider.wallet.publicKey.toBase58());
    console.log('Token0 Mint:', TOKEN0_MINT.toBase58());
    console.log('Token1 Mint:', TOKEN1_MINT.toBase58());

    // Find PDA for the pair
    const [pairPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_pair'), TOKEN0_MINT.toBuffer(), TOKEN1_MINT.toBuffer()],
        program.programId
    );

    // Get pair account to get rate model
    const pairAccount = await program.account.pair.fetch(pairPda);
    const RATE_MODEL = pairAccount.rateModel;

    console.log('Rate Model address:', RATE_MODEL.toBase58());

    // Find PDA for the LP mint
    const [lpMintPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_lp_mint'), pairPda.toBuffer()],
        program.programId
    );

    // Get token program for each mint
    const token0Info = await provider.connection.getAccountInfo(TOKEN0_MINT);
    const token1Info = await provider.connection.getAccountInfo(TOKEN1_MINT);
    const lpMintInfo = await provider.connection.getAccountInfo(lpMintPda);
    
    const token0Program = token0Info?.owner.equals(TOKEN_2022_PROGRAM_ID) 
        ? TOKEN_2022_PROGRAM_ID 
        : TOKEN_PROGRAM_ID;
    const token1Program = token1Info?.owner.equals(TOKEN_2022_PROGRAM_ID) 
        ? TOKEN_2022_PROGRAM_ID 
        : TOKEN_PROGRAM_ID;
    const lpTokenProgram = lpMintInfo?.owner.equals(TOKEN_2022_PROGRAM_ID)
        ? TOKEN_2022_PROGRAM_ID
        : TOKEN_PROGRAM_ID;

    console.log('Token0 Program:', token0Program.toBase58());
    console.log('Token1 Program:', token1Program.toBase58());
    console.log('LP Token Program:', lpTokenProgram.toBase58());

    // Get associated token addresses for vaults
    const token0Vault = await getAssociatedTokenAddress(
        TOKEN0_MINT,
        pairPda,
        true,
        token0Program,
        ASSOCIATED_TOKEN_PROGRAM_ID
    );
    const token1Vault = await getAssociatedTokenAddress(
        TOKEN1_MINT,
        pairPda,
        true,
        token1Program,
        ASSOCIATED_TOKEN_PROGRAM_ID
    );

    // Get LP token account
    const deployerLpTokenAccount = await getAssociatedTokenAddress(
        lpMintPda,
        DEPLOYER_KEYPAIR.publicKey,
        false,
        lpTokenProgram,
        ASSOCIATED_TOKEN_PROGRAM_ID
    );

    const lpTokenAccountInfo = await getAccount(provider.connection, deployerLpTokenAccount);
    console.log('LP Token Account Owner:', lpTokenAccountInfo.owner.toBase58());
    console.log('Expected Owner (Deployer):', DEPLOYER_KEYPAIR.publicKey.toBase58());

    // Get current LP token balance
    const lpBalance = (await getAccount(provider.connection, deployerLpTokenAccount)).amount;
    const removeAmount = new BN(lpBalance.toString()).divn(2); // Remove half of LP tokens
    const minAmount0 = new BN(0); // Minimum amount of token0 to receive
    const minAmount1 = new BN(0); // Minimum amount of token1 to receive

    console.log('Removing liquidity:');
    console.log('LP Amount:', removeAmount.toString());
    console.log('Min Token0:', minAmount0.toString());
    console.log('Min Token1:', minAmount1.toString());
    console.log('LP Token Account:', deployerLpTokenAccount.toBase58());
    console.log('Balance:', lpBalance.toString());
    console.log('userToken0Account:', DEPLOYER_TOKEN0_ACCOUNT.toBase58());
    console.log('userToken1Account:', DEPLOYER_TOKEN1_ACCOUNT.toBase58());

    // Create transaction
    const tx = await program.methods
        .removeLiquidity({
            liquidityIn: removeAmount,
            minAmount0Out: minAmount0,
            minAmount1Out: minAmount1
        })
        .accountsStrict({
            user: DEPLOYER_KEYPAIR.publicKey,
            pair: pairPda,
            rateModel: RATE_MODEL,
            token0Vault: token0Vault,
            token1Vault: token1Vault,
            userToken0Account: DEPLOYER_TOKEN0_ACCOUNT,
            userToken1Account: DEPLOYER_TOKEN1_ACCOUNT,
            token0VaultMint: TOKEN0_MINT,
            token1VaultMint: TOKEN1_MINT,
            lpMint: lpMintPda,
            userLpTokenAccount: deployerLpTokenAccount,
            tokenProgram: lpTokenProgram,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
        })
        .signers([DEPLOYER_KEYPAIR])
        .rpc();

    console.log('Transaction successful!');
    console.log('Signature:', tx);
}

main().catch(error => {
    console.error('Error:', error);
    process.exit(1);
}); 