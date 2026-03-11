import {
    PublicKey,
    Keypair,
    SystemProgram,
} from '@solana/web3.js';
import BN from 'bn.js';
import { Program } from '@coral-xyz/anchor';
import idl from '../target/idl/omnipair.json' with { type: "json" };
import type { Omnipair } from '../target/types/omnipair';
import * as anchor from '@coral-xyz/anchor';
import * as dotenv from 'dotenv';

dotenv.config();

// Hardcoded pair address — update this for each pair you want to re-rate
const PAIR_ADDRESS = new PublicKey('REPLACE_WITH_PAIR_PDA');

// Program defaults (must match constants.rs)
const DEFAULT_TARGET_UTIL_START_BPS = 3_000; // 30%
const DEFAULT_TARGET_UTIL_END_BPS = 5_000;   // 50%
const DEFAULT_RATE_HALF_LIFE_MS = 3 * 86_400_000; // 3 days
const MIN_RATE_OVERRIDE_BPS = 400; // 4% — the new floor we want

const NAD = 1_000_000_000;

function nadToBps(nad: BN): number {
    // bps = nad * 10000 / NAD
    return nad.muln(10_000).div(new BN(NAD)).toNumber();
}

async function main() {
    const provider = anchor.AnchorProvider.env();
    const program = new Program<Omnipair>(idl, provider);
    const authority = provider.wallet.payer;

    if (!authority) {
        throw new Error('Wallet keypair not found');
    }

    provider.opts.commitment = 'confirmed';
    provider.opts.preflightCommitment = 'confirmed';

    // Fetch pair to get current rate model
    const pair = await program.account.pair.fetch(PAIR_ADDRESS);
    console.log('Pair:', PAIR_ADDRESS.toBase58());
    console.log('Current rate model:', pair.rateModel.toBase58());

    // Fetch current rate model
    const currentRM = await program.account.rateModel.fetch(pair.rateModel);
    const currentMinBps = nadToBps(currentRM.minRate);
    const currentMaxBps = nadToBps(currentRM.maxRate);
    const currentInitBps = nadToBps(currentRM.initialRate);
    const currentUtilStart = nadToBps(currentRM.targetUtilStart);
    const currentUtilEnd = nadToBps(currentRM.targetUtilEnd);

    console.log('\n--- Current Rate Model ---');
    console.log(`  util band:     ${currentUtilStart} - ${currentUtilEnd} bps (${currentUtilStart / 100}% - ${currentUtilEnd / 100}%)`);
    console.log(`  half_life_ms:  ${currentRM.halfLifeMs.toString()}`);
    console.log(`  min_rate:      ${currentMinBps} bps (${currentMinBps / 100}%)`);
    console.log(`  max_rate:      ${currentMaxBps} bps (${currentMaxBps / 100}%) ${currentMaxBps === 0 ? '(uncapped)' : ''}`);
    console.log(`  initial_rate:  ${currentInitBps} bps (${currentInitBps / 100}%)`);

    // Build new parameters: clone current, override where needed
    const newUtilStart = DEFAULT_TARGET_UTIL_START_BPS;
    const newUtilEnd = DEFAULT_TARGET_UTIL_END_BPS;
    const newHalfLifeMs = currentRM.halfLifeMs; // keep current
    const newMinRateBps = Math.max(currentMinBps, MIN_RATE_OVERRIDE_BPS);
    const newMaxRateBps = currentMaxBps; // keep current
    // Ensure initial rate is at least the new min
    const newInitialRateBps = Math.max(currentInitBps, newMinRateBps);

    console.log('\n--- New Rate Model ---');
    console.log(`  util band:     ${newUtilStart} - ${newUtilEnd} bps (${newUtilStart / 100}% - ${newUtilEnd / 100}%)`);
    console.log(`  half_life_ms:  ${newHalfLifeMs.toString()}`);
    console.log(`  min_rate:      ${newMinRateBps} bps (${newMinRateBps / 100}%)`);
    console.log(`  max_rate:      ${newMaxRateBps} bps (${newMaxRateBps / 100}%) ${newMaxRateBps === 0 ? '(uncapped)' : ''}`);
    console.log(`  initial_rate:  ${newInitialRateBps} bps (${newInitialRateBps / 100}%)`);

    // Derive futarchy authority PDA
    const [futarchyAuthorityPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('futarchy_authority')],
        program.programId
    );

    // Generate keypair for the new rate model account
    const rateModelKeypair = Keypair.generate();
    console.log('\nNew rate model address:', rateModelKeypair.publicKey.toBase58());

    const tx = await program.methods
        .createRateModel({
            targetUtilStartBps: new BN(newUtilStart),
            targetUtilEndBps: new BN(newUtilEnd),
            halfLifeMs: newHalfLifeMs,
            minRateBps: new BN(newMinRateBps),
            maxRateBps: new BN(newMaxRateBps),
            initialRateBps: new BN(newInitialRateBps),
        })
        .accountsPartial({
            authoritySigner: authority.publicKey,
            futarchyAuthority: futarchyAuthorityPda,
            rateModel: rateModelKeypair.publicKey,
            systemProgram: SystemProgram.programId,
        })
        .signers([authority, rateModelKeypair])
        .rpc();

    console.log('\nRate model created successfully!');
    console.log('Signature:', tx);
    console.log('\nNew rate model address:', rateModelKeypair.publicKey.toBase58());
    console.log('\nTo apply to the pair, call set_pair_rate_model with:');
    console.log(`  pair:           ${PAIR_ADDRESS.toBase58()}`);
    console.log(`  new_rate_model: ${rateModelKeypair.publicKey.toBase58()}`);
}

main().catch(error => {
    console.error('Error:', error);
    process.exit(1);
});
