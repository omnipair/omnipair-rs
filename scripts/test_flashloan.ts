import { 
    PublicKey,
    SystemProgram,
} from '@solana/web3.js';
import { 
    TOKEN_PROGRAM_ID, 
    TOKEN_2022_PROGRAM_ID,
    getAssociatedTokenAddress,
} from '@solana/spl-token';
import { Program } from '@coral-xyz/anchor';
import omnipairIdl from '../target/idl/omnipair.json' with { type: "json" };
import receiverIdl from '../target/idl/flashloan_receiver_example.json' with { type: "json" };
import type { Omnipair } from '../target/types/omnipair';
import type { FlashloanReceiverExample } from '../target/types/flashloan_receiver_example';
import BN from 'bn.js';
import * as anchor from '@coral-xyz/anchor';
import * as dotenv from 'dotenv';

// Load environment variables
dotenv.config();

// Replace these with your actual values
const TOKEN0_MINT = new PublicKey(process.env.TOKEN0_MINT || '');
const TOKEN1_MINT = new PublicKey(process.env.TOKEN1_MINT || '');
const RECEIVER_PROGRAM_ID = new PublicKey('4RedqfMFEYWAujththpBsB3VPPiroUUD21xZULVfZCuR');

async function main() {
    console.log('=== Testing Flash Loan with Receiver Program ===\n');
    
    // Setup connection and provider using Anchor configuration
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    const omnipairProgram = new Program<Omnipair>(omnipairIdl as any, provider);
    const receiverIdlWithAddress = { ...receiverIdl, address: RECEIVER_PROGRAM_ID.toBase58() };
    const receiverProgram = new Program<FlashloanReceiverExample>(receiverIdlWithAddress as any, provider);
    
    const DEPLOYER_KEYPAIR = provider.wallet.payer;
    
    if(!DEPLOYER_KEYPAIR) {
        throw new Error('Deployer keypair not found');
    }

    // Set proper commitment levels
    provider.opts.commitment = 'confirmed';
    provider.opts.preflightCommitment = 'confirmed';
    provider.opts.skipPreflight = false;
    console.log('Connected to network:', provider.connection.rpcEndpoint);
    console.log('User:', DEPLOYER_KEYPAIR.publicKey.toBase58());
    console.log('Token0 Mint:', TOKEN0_MINT.toBase58());
    console.log('Token1 Mint:', TOKEN1_MINT.toBase58());
    console.log('Receiver Program:', receiverProgram.programId.toBase58());
    
    // Find the pair PDA
    const [pairPda] = PublicKey.findProgramAddressSync(
        [
            Buffer.from('gamm_pair'),
            TOKEN0_MINT.toBuffer(),
            TOKEN1_MINT.toBuffer(),
        ],
        omnipairProgram.programId
    );
    console.log('Pair PDA:', pairPda.toBase58());
    
    // Get pair account to find rate model
    const pairAccount = await omnipairProgram.account.pair.fetch(pairPda);
    const rateModel = pairAccount.rateModel;
    console.log('Rate Model:', rateModel.toBase58());
    
    // Get token vaults (ATAs owned by the pair)
    const token0Vault = await getAssociatedTokenAddress(
        TOKEN0_MINT,
        pairPda,
        true
    );
    
    const token1Vault = await getAssociatedTokenAddress(
        TOKEN1_MINT,
        pairPda,
        true
    );
    
    console.log('Token0 Vault:', token0Vault.toBase58());
    console.log('Token1 Vault:', token1Vault.toBase58());
    
    // Get receiver token accounts (where borrowed tokens will be sent)
    // These are owned by the user (initiator)
    const receiverToken0Account = await getAssociatedTokenAddress(
        TOKEN0_MINT,
        DEPLOYER_KEYPAIR.publicKey
    );
    
    const receiverToken1Account = await getAssociatedTokenAddress(
        TOKEN1_MINT,
        DEPLOYER_KEYPAIR.publicKey
    );
    
    console.log('Receiver Token0 Account:', receiverToken0Account.toBase58());
    console.log('Receiver Token1 Account:', receiverToken1Account.toBase58());
    
    // Flash loan parameters
    const flashloanAmount0 = new BN(1_000_000); // 1 token0 (assuming 6 decimals)
    const flashloanAmount1 = new BN(0);         // No token1
    const customData = Buffer.from([]);         // Empty custom data
    
    console.log('\n=== Flash Loan Parameters ===');
    console.log('Borrowing token0:', flashloanAmount0.toString());
    console.log('Borrowing token1:', flashloanAmount1.toString());
    
    // Get balances before
    console.log('\n=== Balances Before ===');
    const receiverToken0Before = await provider.connection.getTokenAccountBalance(receiverToken0Account);
    const receiverToken1Before = await provider.connection.getTokenAccountBalance(receiverToken1Account);
    const vault0Before = await provider.connection.getTokenAccountBalance(token0Vault);
    const vault1Before = await provider.connection.getTokenAccountBalance(token1Vault);
    
    console.log('Receiver Token0:', receiverToken0Before.value.uiAmountString);
    console.log('Receiver Token1:', receiverToken1Before.value.uiAmountString);
    console.log('Vault Token0:', vault0Before.value.uiAmountString);
    console.log('Vault Token1:', vault1Before.value.uiAmountString);
    
    // Check if receiver has enough to return (should have from the callback)
    // In a real scenario, the callback would perform arbitrage and profit
    console.log('\n=== Executing Flash Loan ===');
    
    try {
        const tx = await omnipairProgram.methods
            .flashloan({
                amount0: flashloanAmount0,
                amount1: flashloanAmount1,
                data: customData,
            })
            .accountsPartial({
                pair: pairPda,
                rateModel: rateModel,
                token0Vault: token0Vault,
                token1Vault: token1Vault,
                token0Mint: TOKEN0_MINT,
                token1Mint: TOKEN1_MINT,
                receiverToken0Account: receiverToken0Account,
                receiverToken1Account: receiverToken1Account,
                receiverProgram: receiverProgram.programId,
                user: DEPLOYER_KEYPAIR.publicKey,
                tokenProgram: TOKEN_PROGRAM_ID,
                token2022Program: TOKEN_2022_PROGRAM_ID,
                systemProgram: SystemProgram.programId,
            })
            // Add any additional accounts your receiver needs via remainingAccounts
            .remainingAccounts([
                // The vaults need to be passed as remaining accounts so the receiver can return tokens
                { pubkey: token0Vault, isSigner: false, isWritable: true },
                { pubkey: token1Vault, isSigner: false, isWritable: true },
            ])
            .rpc();
        
        console.log('✅ Flash loan successful!');
        console.log('Transaction signature:', tx);
        
        // Wait for confirmation
        await provider.connection.confirmTransaction(tx, 'confirmed');
        
        // Get balances after
        console.log('\n=== Balances After ===');
        const receiverToken0After = await provider.connection.getTokenAccountBalance(receiverToken0Account);
        const receiverToken1After = await provider.connection.getTokenAccountBalance(receiverToken1Account);
        const vault0After = await provider.connection.getTokenAccountBalance(token0Vault);
        const vault1After = await provider.connection.getTokenAccountBalance(token1Vault);
        
        console.log('Receiver Token0:', receiverToken0After.value.uiAmountString);
        console.log('Receiver Token1:', receiverToken1After.value.uiAmountString);
        console.log('Vault Token0:', vault0After.value.uiAmountString);
        console.log('Vault Token1:', vault1After.value.uiAmountString);
        
        // Verify vaults have same or more tokens
        console.log('\n=== Verification ===');
        const vault0Delta = BigInt(vault0After.value.amount) - BigInt(vault0Before.value.amount);
        const vault1Delta = BigInt(vault1After.value.amount) - BigInt(vault1Before.value.amount);
        
        console.log('Vault0 change:', vault0Delta.toString());
        console.log('Vault1 change:', vault1Delta.toString());
        
        if (vault0Delta >= 0n && vault1Delta >= 0n) {
            console.log('✅ Tokens were returned successfully!');
        } else {
            console.log('⚠️  Warning: Vault balances decreased (unexpected)');
        }
        
        // Fetch and display flash loan event
        console.log('\n=== Flash Loan Event ===');
        const txDetails = await provider.connection.getTransaction(tx, {
            commitment: 'confirmed',
            maxSupportedTransactionVersion: 0,
        });
        
        if (txDetails?.meta?.logMessages) {
            const flashloanLogs = txDetails.meta.logMessages.filter(log => 
                log.includes('Flash Loan') || log.includes('flash loan')
            );
            if (flashloanLogs.length > 0) {
                console.log('Flash loan logs:');
                flashloanLogs.forEach(log => console.log(' ', log));
            }
        }
        
    } catch (error: any) {
        console.error('\n❌ Flash loan failed!');
        
        if (error.logs) {
            console.error('\nProgram logs:');
            error.logs.forEach((log: string) => console.error(' ', log));
        }
        
        if (error.message) {
            console.error('\nError message:', error.message);
        }
        
        // Common errors and solutions
        console.error('\n🔍 Common issues:');
        console.error('1. Receiver program not deployed');
        console.error('   → cd examples/flashloan_receiver && anchor build && anchor deploy');
        console.error('2. Insufficient vault balance');
        console.error('   → Add liquidity to the pair first (yarn bootstrap)');
        console.error('3. Receiver not returning tokens');
        console.error('   → Check receiver program logic');
        console.error('4. Account mismatch');
        console.error('   → Verify token mints and vaults are correct');
        
        throw error;
    }
}

main().catch(error => {
    console.error('\n❌ Script failed:', error);
    process.exit(1);
});
