import { 
    Connection, 
    Keypair,
    Transaction,
    PublicKey
} from '@solana/web3.js';
import { 
    TOKEN_PROGRAM_ID, 
    createMint,

    setAuthority,
    AuthorityType
} from '@solana/spl-token';
import {
    PROGRAM_ID as TOKEN_METADATA_PROGRAM_ID,
    createCreateMetadataAccountV3Instruction,
} from '@metaplex-foundation/mpl-token-metadata';
import idl from '../../target/idl/omnipair.json' with { type: "json" };
import type { Omnipair } from '../../target/types/omnipair';
import * as anchor from '@coral-xyz/anchor';
import * as dotenv from 'dotenv';
import { Program } from '@coral-xyz/anchor';

// Load environment variables
dotenv.config();

async function createMintWithRetry(
    connection: Connection,
    payer: Keypair,
    mintAuthority: PublicKey,
    freezeAuthority: PublicKey | null,
    decimals: number,
    programId = TOKEN_PROGRAM_ID,
    maxRetries = 3
) {
    let lastError;
    for (let i = 0; i < maxRetries; i++) {
        try {
            const mint = await createMint(
                connection,
                payer,
                mintAuthority,
                freezeAuthority,
                decimals,
                undefined,
                undefined,
                programId
            );
            return mint;
        } catch (error) {
            lastError = error;
            console.log(`Attempt ${i + 1} failed, retrying...`);
            await new Promise(resolve => setTimeout(resolve, 2000)); // Wait 2 seconds before retry
        }
    }
    throw lastError;
}

async function createMetadata(
    connection: Connection,
    payer: Keypair,
    mint: PublicKey,
    mintAuthority: PublicKey,
    name: string,
    symbol: string,
    uri: string
) {
    const [metadataAddress] = PublicKey.findProgramAddressSync(
        [
            Buffer.from('metadata'),
            TOKEN_METADATA_PROGRAM_ID.toBuffer(),
            mint.toBuffer(),
        ],
        TOKEN_METADATA_PROGRAM_ID
    );

    const createMetadataInstruction = createCreateMetadataAccountV3Instruction(
        {
            metadata: metadataAddress,
            mint: mint,
            mintAuthority: payer.publicKey,
            payer: payer.publicKey,
            updateAuthority: payer.publicKey,
        },
        {
            createMetadataAccountArgsV3: {
                data: {
                    name: name,
                    symbol: symbol,
                    uri: uri,
                    sellerFeeBasisPoints: 0,
                    creators: null,
                    collection: null,
                    uses: null,
                },
                isMutable: true,
                collectionDetails: null,
            },
        }
    );

    const transaction = new Transaction().add(createMetadataInstruction);
    
    // Get the latest blockhash
    const { blockhash } = await connection.getLatestBlockhash();
    transaction.recentBlockhash = blockhash;
    transaction.feePayer = payer.publicKey;

    // Sign the transaction
    transaction.sign(payer);

    // Send and confirm the transaction
    const signature = await connection.sendRawTransaction(transaction.serialize());
    await connection.confirmTransaction(signature);
    
    return metadataAddress;
}

async function main() {
    console.log('Starting token deployment...');
    
    // Setup connection and provider using Anchor configuration
    const provider = anchor.AnchorProvider.env();
    const program = new Program<Omnipair>(idl, provider);
    const DEPLOYER_KEYPAIR = provider.wallet.payer;
    
    if(!DEPLOYER_KEYPAIR) {
        throw new Error('Deployer keypair not found');
    }

    // Set longer confirmation timeout
    provider.opts.commitment = 'confirmed';
    provider.opts.preflightCommitment = 'confirmed';
    provider.opts.skipPreflight = false;

    console.log('Connected to network:', provider.connection.rpcEndpoint);
    console.log('Deployer address:', provider.wallet.publicKey.toBase58());

    // Get program ID from environment
    console.log('Program ID:', program.programId.toBase58());

    const [mintAuthorityPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('faucet_authority'), program.programId.toBuffer()],
        program.programId
    );

    // Create Token0 with deployer as mint authority
    console.log('\nCreating Token0...');
    const token0Mint = await createMintWithRetry(
        provider.connection,
        DEPLOYER_KEYPAIR,
        DEPLOYER_KEYPAIR.publicKey, // Use deployer as mint authority initially
        null, // No freeze authority
        6
    );
    console.log('Token0 Mint:', token0Mint.toBase58());

    // Create Token1 with deployer as mint authority
    console.log('\nCreating Token1...');
    const token1Mint = await createMintWithRetry(
        provider.connection,
        DEPLOYER_KEYPAIR,
        DEPLOYER_KEYPAIR.publicKey, // Use deployer as mint authority initially
        null, // No freeze authority
        6
    );
    console.log('Token1 Mint:', token1Mint.toBase58());

    // Create metadata for Token0
    console.log('\nCreating metadata for Token0...');
    const token0Metadata = await createMetadata(
        provider.connection,
        DEPLOYER_KEYPAIR,
        token0Mint,
        DEPLOYER_KEYPAIR.publicKey,
        'MetaDAO',
        'META',
        'https://bafybeighnmau6rofxubsg3hroby3cclodur53zr25araufrgorzkwljdxy.ipfs.w3s.link/metadao_metadata.json'
    );
    console.log('Token0 Metadata:', token0Metadata.toBase58());

    // Create metadata for Token1
    console.log('\nCreating metadata for Token1...');
    const token1Metadata = await createMetadata(
        provider.connection,
        DEPLOYER_KEYPAIR,
        token1Mint,
        DEPLOYER_KEYPAIR.publicKey,
        'USD Coin',
        'USDC',
        'https://bafybeiaycwfghj7ap7i2g5jfhtxihdulegikgq45dzcau3zhqkvg2zrnvm.ipfs.w3s.link/usdc_metadata.json'
    );
    console.log('Token1 Metadata:', token1Metadata.toBase58());

    // Transfer mint authority to PDA for Token0
    console.log('\nTransferring Token0 mint authority to PDA...');
    await setAuthority(
        provider.connection,
        DEPLOYER_KEYPAIR,
        token0Mint,
        DEPLOYER_KEYPAIR.publicKey,
        AuthorityType.MintTokens,
        mintAuthorityPda
    );
    console.log('Token0 mint authority transferred to PDA');

    // Transfer mint authority to PDA for Token1
    console.log('\nTransferring Token1 mint authority to PDA...');
    await setAuthority(
        provider.connection,
        DEPLOYER_KEYPAIR,
        token1Mint,
        DEPLOYER_KEYPAIR.publicKey,
        AuthorityType.MintTokens,
        mintAuthorityPda
    );
    console.log('Token1 mint authority transferred to PDA');

    console.log('\nToken deployment completed successfully!');
    console.log('Token0 Mint:', token0Mint.toBase58());
    console.log('Token1 Mint:', token1Mint.toBase58());
    console.log('Token0 Metadata:', token0Metadata.toBase58());
    console.log('Token1 Metadata:', token1Metadata.toBase58());
}

main().catch(error => {
    console.error('Error:', error);
    process.exit(1);
});
