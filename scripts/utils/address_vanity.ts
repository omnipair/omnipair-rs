import { PublicKey } from '@solana/web3.js';
import * as dotenv from 'dotenv';

// Load environment variables
dotenv.config();

async function main() {
    const baseAddress = new PublicKey(process.env.BASE_ADDRESS || 'C7GKpfqQyBoFR6S13DECwBjdi7aCQKbbeKjXm4Jt5Hds');
    const programId = new PublicKey('BPFLoaderUpgradeab1e11111111111111111111111');
    const seed = 'vvFfCduOi9yT4iot'; // This is a string, not bytes!

    // Derive the address with seed (same as createAddressWithSeed)
    const derivedAddress = await PublicKey.createWithSeed(
        baseAddress,
        seed,
        programId
    );

    console.log('Derived Address (base58):', derivedAddress.toBase58());

    // Log the bytes of the derived public key
    const pubkeyBytes = derivedAddress.toBytes();
    const byteArray = Array.from(pubkeyBytes);
    console.log('Derived Address Bytes:', `[${byteArray.join(', ')}]`);
}

main().catch(console.error);
