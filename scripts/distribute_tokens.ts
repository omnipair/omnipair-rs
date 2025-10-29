import { 
    PublicKey, 
} from '@solana/web3.js';
import { Program } from '@coral-xyz/anchor';
import idl from '../target/idl/omnipair.json' with { type: "json" };
import type { Omnipair } from '../target/types/omnipair';
import * as anchor from '@coral-xyz/anchor';
import * as dotenv from 'dotenv';
import { getAssociatedTokenAddress, TOKEN_PROGRAM_ID } from '@solana/spl-token';

// Load environment variables
dotenv.config();

async function main() {
    console.log('Starting token distribution...');
    
    // Setup connection and provider using Anchor configuration
    const provider = anchor.AnchorProvider.env();
    const program = new Program<Omnipair>(idl, provider);
    
    // Set proper commitment levels
    provider.opts.commitment = 'confirmed';
    provider.opts.preflightCommitment = 'confirmed';
    provider.opts.skipPreflight = false;

    console.log('Connected to network:', provider.connection.rpcEndpoint);

    // Get the mint address from environment (e.g., USDC mint)
    const MINT_ADDRESS = new PublicKey(process.env.TOKEN_MINT || '');
    console.log('Token Mint:', MINT_ADDRESS.toBase58());

    // Find PDA for futarchy authority
    const [futarchyAuthorityPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('futarchy_authority')],
        program.programId
    );
    console.log('Futarchy Authority PDA:', futarchyAuthorityPda.toBase58());

    // Fetch futarchy authority to get recipients
    const futarchyAuthority = await program.account.futarchyAuthority.fetch(futarchyAuthorityPda);
    console.log('Futarchy Treasury:', futarchyAuthority.recipients.futarchyTreasury.toBase58(), `(${futarchyAuthority.revenueDistribution.futarchyTreasuryBps / 100}%)`);
    console.log('Buybacks Vault:', futarchyAuthority.recipients.buybacksVault.toBase58(), `(${futarchyAuthority.revenueDistribution.buybacksVaultBps / 100}%)`);
    console.log('Team Treasury:', futarchyAuthority.recipients.teamTreasury.toBase58(), `(${futarchyAuthority.revenueDistribution.teamTreasuryBps / 100}%)`);

    // Get associated token addresses for recipients
    const futarchyTreasuryAta = await getAssociatedTokenAddress(
        MINT_ADDRESS,
        futarchyAuthority.recipients.futarchyTreasury,
        false,
        TOKEN_PROGRAM_ID
    );
    const buybacksVaultAta = await getAssociatedTokenAddress(
        MINT_ADDRESS,
        futarchyAuthority.recipients.buybacksVault,
        false,
        TOKEN_PROGRAM_ID
    );
    const teamTreasuryAta = await getAssociatedTokenAddress(
        MINT_ADDRESS,
        futarchyAuthority.recipients.teamTreasury,
        false,
        TOKEN_PROGRAM_ID
    );

    console.log('Futarchy Treasury ATA:', futarchyTreasuryAta.toBase58());
    console.log('Buybacks Vault ATA:', buybacksVaultAta.toBase58());
    console.log('Team Treasury ATA:', teamTreasuryAta.toBase58());

    // Get the source token account (PDA-owned)
    const sourceTokenAccount = await getAssociatedTokenAddress(
        MINT_ADDRESS,
        futarchyAuthorityPda,
        true, // allowOwnerOffCurve for PDAs
        TOKEN_PROGRAM_ID
    );
    console.log('Source Token Account (PDA-owned):', sourceTokenAccount.toBase58());

    // Check balance before distribution
    try {
        const balance = await provider.connection.getTokenAccountBalance(sourceTokenAccount);
        console.log('Source Token Account Balance:', balance.value.uiAmount, balance.value.uiAmountString);
    } catch (error) {
        console.log('Source token account may not exist or have no balance:', error);
    }

    // Distribute tokens
    console.log('Distributing tokens...');
    const tx = await program.methods
        .distributeTokens({})
        .accountsPartial({
            futarchyAuthority: futarchyAuthorityPda,
            sourceMint: MINT_ADDRESS,
            sourceTokenAccount: sourceTokenAccount,
            futarchyTreasuryTokenAccount: futarchyTreasuryAta,
            buybacksVaultTokenAccount: buybacksVaultAta,
            teamTreasuryTokenAccount: teamTreasuryAta,
            tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();
    
    console.log('Distribution transaction:', tx);
    console.log('Tokens distributed successfully!');
    
    // Check balances after distribution
    console.log('\nChecking balances after distribution...');
    try {
        const balance = await provider.connection.getTokenAccountBalance(sourceTokenAccount);
        console.log('Source Token Account Balance:', balance.value.uiAmount, balance.value.uiAmountString);
    } catch (error) {
        console.log('Source token account may not exist or have no balance:', error);
    }
}

main().catch(error => {
    console.error('Error:', error);
    process.exit(1);
});

