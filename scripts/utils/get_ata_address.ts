import { PublicKey } from '@solana/web3.js';
import { getAssociatedTokenAddress } from '@solana/spl-token';
import * as dotenv from 'dotenv';

// Load environment variables
dotenv.config();

async function main() {
    // Get addresses from environment variables or use defaults
    const ownerAddress = new PublicKey(process.env.OWNER_ADDRESS || 'C7GKpfqQyBoFR6S13DECwBjdi7aCQKbbeKjXm4Jt5Hds');
    const mintAddress = new PublicKey(process.env.MINT_ADDRESS || '4QQ5XxM7tn4bAXUp2dBYFRfi2VdScEv9dzw82oMo8ko4');

    const ataAddress = await getAssociatedTokenAddress(
        mintAddress,
        ownerAddress
    );

    console.log('Associated Token Account Address:', ataAddress.toBase58());
}

main().catch(console.error);