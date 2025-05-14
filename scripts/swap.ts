import { 
    PublicKey, 
} from '@solana/web3.js';
import { 
    TOKEN_PROGRAM_ID, 
    TOKEN_2022_PROGRAM_ID,
    getAssociatedTokenAddress,
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
    console.log('Starting token swap...');
    
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

    console.log('Connected to network:', provider.connection.rpcEndpoint);
    console.log('Deployer address:', provider.wallet.publicKey.toBase58());
    console.log('Token0 Account:', DEPLOYER_TOKEN0_ACCOUNT.toBase58());
    console.log('Token1 Account:', DEPLOYER_TOKEN1_ACCOUNT.toBase58());

    // Find PDA for the pair
    const [pairPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_pair'), TOKEN0_MINT.toBuffer(), TOKEN1_MINT.toBuffer()],
        program.programId
    );
    console.log('Pair PDA:', pairPda.toBase58());
    const pairAccount = await program.account.pair.fetch(pairPda);

    // Get token program for each mint
    const token0Info = await provider.connection.getAccountInfo(TOKEN0_MINT);
    const token1Info = await provider.connection.getAccountInfo(TOKEN1_MINT);
    
    const token0Program = token0Info?.owner.equals(TOKEN_2022_PROGRAM_ID) 
        ? TOKEN_2022_PROGRAM_ID 
        : TOKEN_PROGRAM_ID;
    const token1Program = token1Info?.owner.equals(TOKEN_2022_PROGRAM_ID) 
        ? TOKEN_2022_PROGRAM_ID 
        : TOKEN_PROGRAM_ID;

    // Get associated token addresses for vaults
    const token0Vault = await getAssociatedTokenAddress(
        TOKEN0_MINT,
        pairPda,
        true,
        token0Program
    );
    const token1Vault = await getAssociatedTokenAddress(
        TOKEN1_MINT,
        pairPda,
        true,
        token1Program
    );
    console.log('Token0 Vault:', token0Vault.toBase58());
    console.log('Token1 Vault:', token1Vault.toBase58());

    // Swap parameters
    const amountIn = new BN(1000_000_000); // Amount of token0 to swap
    const minAmountOut = new BN(0); // Minimum amount of token1 to receive

    console.log('Swap parameters:');
    console.log('Amount In:', amountIn.toString());
    console.log('Min Amount Out:', minAmountOut.toString());

    // Create transaction
    const tx = await program.methods
        .swap({
            amountIn: amountIn,
            minAmountOut: minAmountOut,
        })
        .accountsPartial({
            user: DEPLOYER_KEYPAIR.publicKey,
            pair: pairPda,
            rateModel: pairAccount.rateModel,
            tokenInVault: token0Vault,
            tokenOutVault: token1Vault,
            userTokenInAccount: DEPLOYER_TOKEN0_ACCOUNT,
            userTokenOutAccount: DEPLOYER_TOKEN1_ACCOUNT,
            tokenInMint: TOKEN0_MINT,
            tokenOutMint: TOKEN1_MINT,
        })
        .signers([DEPLOYER_KEYPAIR])
        .rpc();

    console.log('Transaction successful!');
    console.log('Signature:', tx);
}

main().catch(console.error); 