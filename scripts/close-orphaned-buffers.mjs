#!/usr/bin/env node
/**
 * Finds and closes program buffers whose authority is a Squads vault.
 *
 * Buffer rent is only refunded when an upgrade executes: the loader closes the
 * buffer and sends its lamports to the spill account named in that instruction.
 * A proposal that is cancelled, expired, or superseded leaves the buffer fully
 * funded forever, so buffers accumulate silently at roughly 8 SOL each.
 *
 * The vault holds the authority, so closing them needs a Squads proposal. This
 * prints a base58 transaction message to paste into the Squads UI.
 *
 * Usage:
 *   RPC_URL=<rpc> VAULT=<vault address> node scripts/close-orphaned-buffers.mjs
 *   RPC_URL=<rpc> VAULT=<vault> RECIPIENT=<addr> node scripts/close-orphaned-buffers.mjs
 *
 * RECIPIENT defaults to the vault itself.
 */
import {
  Connection,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js"
import bs58 from "bs58"

const RPC_URL = process.env.RPC_URL
const VAULT = process.env.VAULT
const RECIPIENT = process.env.RECIPIENT || VAULT

if (!RPC_URL || !VAULT) {
  console.error("Set RPC_URL and VAULT. See the header of this file.")
  process.exit(1)
}

const LOADER = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111")
/** BPFLoaderUpgradeable instruction index for Close. */
const CLOSE_IX = 5
/** UpgradeableLoaderState::Buffer discriminant, as a 4-byte LE enum tag. */
const BUFFER_TAG = "2" // base58 of [1, 0, 0, 0]

const connection = new Connection(RPC_URL, "confirmed")
const vault = new PublicKey(VAULT)
const recipient = new PublicKey(RECIPIENT)

// Buffer layout: [0..4] enum tag, [4] Option flag, [5..37] authority
const accounts = await connection.getProgramAccounts(LOADER, {
  dataSlice: { offset: 0, length: 37 },
  filters: [
    { memcmp: { offset: 0, bytes: BUFFER_TAG } },
    { memcmp: { offset: 5, bytes: vault.toBase58() } },
  ],
})

if (accounts.length === 0) {
  console.log(`No buffers found with authority ${vault.toBase58()}`)
  process.exit(0)
}

console.log(`Buffers with authority ${vault.toBase58()}:\n`)
let total = 0
const instructions = []

for (const { pubkey } of accounts) {
  const info = await connection.getAccountInfo(pubkey)
  total += info.lamports
  console.log(
    `  ${pubkey.toBase58()}  ${(info.lamports / 1e9).toFixed(3)} SOL  ${info.data.length} bytes`,
  )

  instructions.push(
    new TransactionInstruction({
      programId: LOADER,
      keys: [
        { pubkey, isSigner: false, isWritable: true },
        { pubkey: recipient, isSigner: false, isWritable: true },
        { pubkey: vault, isSigner: true, isWritable: false },
      ],
      data: Buffer.from(new Uint32Array([CLOSE_IX]).buffer),
    }),
  )
}

console.log(`\nReclaimable: ${(total / 1e9).toFixed(3)} SOL -> ${recipient.toBase58()}`)

const { blockhash } = await connection.getLatestBlockhash()
const message = new TransactionMessage({
  payerKey: vault,
  recentBlockhash: blockhash,
  instructions,
}).compileToV0Message()

console.log(
  `\nBase58 transaction message for the Squads UI (Create transaction -> import):\n`,
)
console.log(bs58.encode(new VersionedTransaction(message).serialize()))
console.log(
  `\nThe vault is the only signer, so this needs no extra keypair. Approve and execute in Squads as normal.`,
)
