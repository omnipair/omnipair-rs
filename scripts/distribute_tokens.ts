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
    console.log('Recipient 1:', futarchyAuthority.recipient1.toBase58(), `(${futarchyAuthority.recipient1PercentageBps / 100}%)`);
    console.log('Recipient 2:', futarchyAuthority.recipient2.toBase58(), `(${futarchyAuthority.recipient2PercentageBps / 100}%)`);
    console.log('Recipient 3:', futarchyAuthority.recipient3.toBase58(), `(${futarchyAuthority.recipient3PercentageBps / 100}%)`);

    // Get associated token addresses for recipients
    const recipient1Ata = await getAssociatedTokenAddress(
        MINT_ADDRESS,
        futarchyAuthority.recipient1,
        false,
        TOKEN_PROGRAM_ID
    );
    const recipient2Ata = await getAssociatedTokenAddress(
        MINT_ADDRESS,
        futarchyAuthority.recipient2,
        false,
        TOKEN_PROGRAM_ID
    );
    const recipient3Ata = await getAssociatedTokenAddress(
        MINT_ADDRESS,
        futarchyAuthority.recipient3,
        false,
        TOKEN_PROGRAM_ID
    );

    console.log('Recipient 1 ATA:', recipient1Ata.toBase58());
    console.log('Recipient 2 ATA:', recipient2Ata.toBase58());
    console.log('Recipient 3 ATA:', recipient3Ata.toBase58());

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
            sourceTokenAccount: sourceTokenAccount,
            recipient1TokenAccount: recipient1Ata,
            recipient2TokenAccount: recipient2Ata,
            recipient3TokenAccount: recipient3Ata,
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

