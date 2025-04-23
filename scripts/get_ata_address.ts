import { PublicKey } from '@solana/web3.js';
import { getAssociatedTokenAddress } from '@solana/spl-token';

async function main() {
    // Replace these with your actual addresses
    const ownerAddress = new PublicKey('C7GKpfqQyBoFR6S13DECwBjdi7aCQKbbeKjXm4Jt5Hds');
    const mintAddress = new PublicKey('4QQ5XxM7tn4bAXUp2dBYFRfi2VdScEv9dzw82oMo8ko4');

    const ataAddress = await getAssociatedTokenAddress(
        mintAddress,
        ownerAddress
    );

    console.log('Associated Token Account Address:', ataAddress.toBase58());
}

main().catch(console.error); 