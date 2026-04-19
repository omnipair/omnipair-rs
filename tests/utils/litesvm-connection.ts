import { PublicKey, Keypair, Connection, Commitment, TransactionConfirmationStrategy, RpcResponseAndContext, SignatureResult, Transaction, VersionedTransaction, Signer, SendOptions } from "@solana/web3.js";
import { LiteSVM } from "litesvm";

// Create a Connection wrapper for LiteSVM that intercepts all calls
export class LiteSVMConnection extends Connection {
  constructor(private svm: LiteSVM) {
    // Use a dummy URL - we'll override all methods anyway
    super("http://localhost:8899", "confirmed");
    // Override the internal _rpcRequest to handle RPC calls
    (this as any)._rpcRequest = async (method: string, args: any[]) => {
      // Handle getAccountInfo RPC calls
      if (method === "getAccountInfo") {
        const address = new PublicKey(args[0]);
        const accountInfo = await this.getAccountInfo(address, args[1]);
        return {
          context: { slot: 0 },
          value: accountInfo
        };
      }
      // Handle getBalance RPC calls
      if (method === "getBalance") {
        const address = new PublicKey(args[0]);
        const balance = await this.getBalance(address, args[1]);
        return {
          context: { slot: 0 },
          value: balance
        };
      }
      // Handle getMinimumBalanceForRentExemption
      if (method === "getMinimumBalanceForRentExemption") {
        const dataLength = args[0];
        const minBalance = await this.getMinimumBalanceForRentExemption(dataLength, args[1]);
        return minBalance;
      }
      // Handle getLatestBlockhash
      if (method === "getLatestBlockhash") {
        return await this.getLatestBlockhash(args[0]);
      }
      // Handle other RPC methods that we don't support yet
      throw new Error(`Unsupported RPC method: ${method}. LiteSVM connection does not support HTTP RPC calls.`);
    };
  }

  // Overload signatures to match Connection interface
  async sendTransaction(
    transaction: Transaction,
    signers: Signer[],
    options?: SendOptions
  ): Promise<string>;
  async sendTransaction(
    transaction: VersionedTransaction,
    options?: SendOptions
  ): Promise<string>;
  async sendTransaction(
    transaction: Transaction | VersionedTransaction,
    signersOrOptions?: Signer[] | SendOptions,
    maybeOptions?: SendOptions
  ): Promise<string> {
    // Handle both overload cases
    let signers: Signer[] | undefined;
    let options: SendOptions | undefined;

    if (Array.isArray(signersOrOptions)) {
      signers = signersOrOptions;
      options = maybeOptions;
    } else {
      options = signersOrOptions;
    }

    // Ensure transaction has a recent blockhash
    if (transaction instanceof Transaction) {
      if (!transaction.recentBlockhash) {
        const { blockhash } = await this.getLatestBlockhash();
        transaction.recentBlockhash = blockhash;
      }
      // Sign the transaction if signers are provided
      if (signers) {
        transaction.sign(...signers);
      }
    }
    
    const result = this.svm.sendTransaction(transaction);
    
    // Check if result has err method (FailedTransactionMetadata)
    if (result && typeof (result as any).err === 'function') {
      const err = (result as any).err();
      const logs = (result as any).meta?.()?.logs?.() || [];
      const errMsg = err ? (typeof err === 'string' ? err : JSON.stringify(err)) : 
        (logs.length > 0 ? `Transaction failed. Logs: ${logs.join('\n')}` : "Transaction failed");
      throw new Error(`Transaction failed: ${errMsg}`);
    }
    
    // Check if result has err property
    if (result && "err" in result && result.err) {
      const errMsg = typeof result.err === 'string' ? result.err : JSON.stringify(result.err);
      throw new Error(`Transaction failed: ${errMsg}`);
    }
    
    // Return a dummy signature - we'll use the transaction signature if available
    if (transaction instanceof Transaction && transaction.signature) {
      return Buffer.from(transaction.signature).toString('base64');
    }
    return "signature";
  }

  async sendRawTransaction(raw: Buffer, options?: any): Promise<string> {
    // Deserialize and send
    try {
      const tx = Transaction.from(raw);
      // Get blockhash if not set
      if (!tx.recentBlockhash) {
        const { blockhash } = await this.getLatestBlockhash();
        tx.recentBlockhash = blockhash;
      }
      // Transaction should already be signed, but verify
      if (!tx.signature) {
        throw new Error("Transaction is not signed");
      }
      const result = this.svm.sendTransaction(tx);
      
      // Check if result has err method (FailedTransactionMetadata)
      if (result && typeof (result as any).err === 'function') {
        const err = (result as any).err();
        const meta = (result as any).meta?.();
        const logs = meta?.logs?.() || [];
        const errStr = err ? err.toString() : "Unknown error";
        const logStr = logs.length > 0 ? `\nLogs: ${logs.join('\n')}` : "";
        throw new Error(`Transaction failed: ${errStr}${logStr}`);
      }
      
      // Check if result has err property
      if (result && "err" in result && result.err) {
        const errMsg = typeof result.err === 'string' ? result.err : JSON.stringify(result.err);
        throw new Error(`Transaction failed: ${errMsg}`);
      }
      
      // Check if result has signature method (success case)
      if (result && typeof (result as any).signature === 'function') {
        // Success - return signature
        return Buffer.from((result as any).signature()).toString('base64');
      }
    } catch (e: any) {
      throw new Error(`Failed to send raw transaction: ${e.message || e}`);
    }
  }

  async getLatestBlockhash(commitment?: Commitment): Promise<{ blockhash: string; lastValidBlockHeight: number }> {
    return {
      blockhash: this.svm.latestBlockhash(),
      lastValidBlockHeight: 0
    };
  }

  async confirmTransaction(
    strategy: TransactionConfirmationStrategy | string,
    commitment?: Commitment
  ): Promise<RpcResponseAndContext<SignatureResult>> {
    // LiteSVM transactions are immediately confirmed
    return { 
      value: { err: null },
      context: { slot: 0 }
    };
  }

  async requestAirdrop(to: PublicKey, lamports: number): Promise<string> {
    const result = this.svm.airdrop(to, BigInt(lamports));
    if (result && "err" in result) {
      throw new Error(`Airdrop failed: ${JSON.stringify(result.err)}`);
    }
    return "signature";
  }

  async getAccountInfo(address: PublicKey, commitment?: Commitment): Promise<any> {
    const account = this.svm.getAccount(address);
    if (!account) return null;
    
    // account is already AccountInfoBytes (AccountInfo<Uint8Array>)
    // Convert owner bytes to PublicKey
    const owner = new PublicKey(account.owner);
    
    // Ensure data is a Buffer
    const data = Buffer.from(account.data);
    
    // Convert lamports to number if it's a bigint
    const lamports = typeof account.lamports === 'bigint' ? Number(account.lamports) : account.lamports;
    
    return {
      executable: account.executable,
      owner: owner,
      lamports: lamports,
      data: data,
      rentEpoch: 0
    };
  }

  async getBalance(address: PublicKey, commitment?: Commitment): Promise<number> {
    const balance = this.svm.getBalance(address);
    return balance ? Number(balance) : 0;
  }

  async getMinimumBalanceForRentExemption(dataLength: number, commitment?: Commitment): Promise<number> {
    const minBalance = this.svm.minimumBalanceForRentExemption(BigInt(dataLength));
    return Number(minBalance);
  }

  // Override getProgramAccounts to prevent HTTP calls
  async getProgramAccounts(programId: PublicKey, configOrCommitment?: any): Promise<any> {
    return {
      context: { slot: 0 },
      value: []
    };
  }

  // Override simulateTransaction
  async simulateTransaction(): Promise<any> {
    return { value: { err: null } };
  }
}




