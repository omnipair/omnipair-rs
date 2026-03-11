import { 
    PublicKey, 
    SystemProgram,
    Keypair,
    Transaction,
} from '@solana/web3.js';
import { Program } from '@coral-xyz/anchor';
import idl from '../target/idl/omnipair.json' with { type: "json" };
import type { Omnipair } from '../target/types/omnipair';
import * as anchor from '@coral-xyz/anchor';
import * as dotenv from 'dotenv';

// Load environment variables
dotenv.config();

// BPF Loader Upgradeable Program ID
const BPF_LOADER_UPGRADEABLE_ID = new PublicKey('BPFLoaderUpgradeab1e11111111111111111111111');

async function main() {
    console.log('Starting futarchy authority initialization...');
    
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

    // Find PDA for futarchy authority
    const [futarchyAuthorityPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('futarchy_authority')],
        program.programId
    );
    console.log('Futarchy Authority PDA:', futarchyAuthorityPda.toBase58());

    // Step 1: Initialize futarchy authority (if not already initialized)
    try {
        console.log('Initializing futarchy authority...');
        
        // Define treasury accounts and their percentages (must sum to 100%)
        // Futarchy treasury is the futarchy authority PDA itself
        const futarchyTreasury = futarchyAuthorityPda;
        
        // Generate keypairs for buybacks vault and team treasury (or use from env if provided)
        const buybacksVaultKeypair = process.env.BUYBACKS_VAULT_ADDRESS 
            ? null // Will use PublicKey from env
            : Keypair.generate();
        const buybacksVault = process.env.BUYBACKS_VAULT_ADDRESS 
            ? new PublicKey(process.env.BUYBACKS_VAULT_ADDRESS)
            : buybacksVaultKeypair!.publicKey;
        
        const teamTreasuryKeypair = process.env.TEAM_TREASURY_ADDRESS 
            ? null // Will use PublicKey from env
            : Keypair.generate();
        const teamTreasury = process.env.TEAM_TREASURY_ADDRESS 
            ? new PublicKey(process.env.TEAM_TREASURY_ADDRESS)
            : teamTreasuryKeypair!.publicKey;
        
        console.log('Futarchy Treasury (30%):', futarchyTreasury.toBase58());
        console.log('Buybacks Vault (60%):', buybacksVault.toBase58());
        if (buybacksVaultKeypair) {
            console.log('⚠️  Buybacks Vault keypair generated. Save this keypair securely!');
            console.log('   Private key (base58):', Buffer.from(buybacksVaultKeypair.secretKey).toString('base64'));
        }
        console.log('Team Treasury (10%):', teamTreasury.toBase58());
        if (teamTreasuryKeypair) {
            console.log('⚠️  Team Treasury keypair generated. Save this keypair securely!');
            console.log('   Private key (base58):', Buffer.from(teamTreasuryKeypair.secretKey).toString('base64'));
        }
        
        // Create accounts for buybacks vault and team treasury if they were generated
        const signers: Keypair[] = [DEPLOYER_KEYPAIR];
        if (buybacksVaultKeypair || teamTreasuryKeypair) {
            console.log('Creating accounts for generated vaults...');
            const createAccountsTx = new Transaction();
            
            if (buybacksVaultKeypair) {
                const rentExemptAmount = await provider.connection.getMinimumBalanceForRentExemption(0);
                createAccountsTx.add(
                    SystemProgram.createAccount({
                        fromPubkey: DEPLOYER_KEYPAIR.publicKey,
                        newAccountPubkey: buybacksVault,
                        lamports: rentExemptAmount,
                        space: 0,
                        programId: SystemProgram.programId,
                    })
                );
                signers.push(buybacksVaultKeypair);
            }
            
            if (teamTreasuryKeypair) {
                const rentExemptAmount = await provider.connection.getMinimumBalanceForRentExemption(0);
                createAccountsTx.add(
                    SystemProgram.createAccount({
                        fromPubkey: DEPLOYER_KEYPAIR.publicKey,
                        newAccountPubkey: teamTreasury,
                        lamports: rentExemptAmount,
                        space: 0,
                        programId: SystemProgram.programId,
                    })
                );
                signers.push(teamTreasuryKeypair);
            }
            
            if (createAccountsTx.instructions.length > 0) {
                const createAccountsSig = await provider.sendAndConfirm(createAccountsTx, signers);
                console.log('Accounts created:', createAccountsSig);
            }
        }
        
        // Derive program data address (PDA of the program ID under BPF Loader Upgradeable)
        const [programDataAddress] = PublicKey.findProgramAddressSync(
            [program.programId.toBuffer()],
            BPF_LOADER_UPGRADEABLE_ID
        );
        console.log('Program Data Address:', programDataAddress.toBase58());

        const futarchyTx = await program.methods
            .initFutarchyAuthority({
                authority: DEPLOYER_KEYPAIR.publicKey,
                swapBps: 100, // 1% swap fee (100 basis points)
                interestBps: 100, // 1% interest fee (100 basis points)
                futarchyTreasury: futarchyTreasury,
                futarchyTreasuryBps: 3000, // 30%
                buybacksVault: buybacksVault,
                buybacksVaultBps: 6000, // 60%
                teamTreasury: teamTreasury,
                teamTreasuryBps: 1000, // 10%
            })
            .accounts({
                deployer: DEPLOYER_KEYPAIR.publicKey,
                futarchyAuthority: futarchyAuthorityPda,
                programData: programDataAddress,
                systemProgram: SystemProgram.programId,
            })
            .signers([DEPLOYER_KEYPAIR])
            .rpc();
        console.log('Futarchy authority initialized:', futarchyTx);
    } catch (error) {
        console.log('Futarchy authority may already be initialized:', error);
    }

    console.log('Initialization successful!');
    console.log('Futarchy Authority PDA:', futarchyAuthorityPda.toBase58());
}

main().catch(error => {
    console.error('Error:', error);
    process.exit(1);
});
