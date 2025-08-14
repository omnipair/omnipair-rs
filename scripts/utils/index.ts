import { Connection, Transaction, Keypair, sendAndConfirmTransaction } from '@solana/web3.js';
import BN from 'bn.js';

const leU64 = (n: number | BN) =>
    Buffer.from(new BN(n).toArray('le', 8));

async function sendTransactionWithRetry(
    connection: Connection,
    tx: Transaction,
    signers: Keypair[],
    maxRetries = 3
) {
    let lastError;
    for (let i = 0; i < maxRetries; i++) {
        try {
            const signature = await sendAndConfirmTransaction(
                connection,
                tx,
                signers
            );
            console.log('Transaction successful!');
            return signature;
        } catch (error) {
            // Check if the error is because the account already exists
            if (error.message.includes('already in use')) {
                console.log('Transaction successful - account already exists');
                return error.signature || '';
            }
            lastError = error;
            console.log(`Attempt ${i + 1} failed, retrying...`);
            await new Promise(resolve => setTimeout(resolve, 5000)); // Wait 2 seconds before retry
        }
    }
    throw lastError;
}

export { sendTransactionWithRetry, leU64 };