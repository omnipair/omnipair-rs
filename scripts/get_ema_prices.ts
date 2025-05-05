import {
    Connection,
    PublicKey,
  } from '@solana/web3.js';
  import {
    Program,
    AnchorProvider,
  } from '@coral-xyz/anchor';
  import idl from '../target/idl/omnipair.json' with { type: 'json' };
  import type { Omnipair } from '../target/types/omnipair';
  import * as anchor from '@coral-xyz/anchor';
  import * as dotenv from 'dotenv';
  
  dotenv.config();
  
  // === Load environment ===
  const TOKEN0_MINT = new PublicKey(process.env.TOKEN0_MINT || '');
  const TOKEN1_MINT = new PublicKey(process.env.TOKEN1_MINT || '');
  
  async function simulateGetter(
    program: Program<Omnipair>,
    pairPda: PublicKey,
    getter: any // Enum variant object
  ): Promise<{ label: string; value: string }> {
    const sim = await program.methods
      .emitValue(getter)
      .accounts({ pair: pairPda })
      .simulate();
  
    const logs = sim.raw ?? [];

    const label = Object.keys(getter)[0]; // e.g. "price0Nad"
  
    const match = logs
      .map((log) => log.match(new RegExp(`${label}:\\s*(\\d+)`, 'i')))
      .find(Boolean);

    console.log(match);
  
    if (!match || !match[1]) {
      throw new Error(`Value for ${label} not found in logs`);
    }
  
    return { label, value: match[1] };
  }
  
  async function main() {
    const provider = anchor.AnchorProvider.env();
    const program = new Program<Omnipair>(idl, provider);
  
    const [pairPda] = PublicKey.findProgramAddressSync(
      [Buffer.from('gamm_pair'), TOKEN0_MINT.toBuffer(), TOKEN1_MINT.toBuffer()],
      program.programId
    );
  
    console.log('Simulating on-chain prices for pair:', pairPda.toBase58());
  
    // notice its written camelCase not PascalCase
    // although PascalCase is used in the idl
    // it will through an error if you use PascalCase
    const enumVariants = [
      { emaPrice0Nad: {} },
      { emaPrice1Nad: {} },
      { spotPrice0Nad: {} },
      { spotPrice1Nad: {} },
    ];
  
    for (const getter of enumVariants) {
      const { label, value } = await simulateGetter(program, pairPda, getter);
      console.log(`${label}: ${value}`);
    }
  }
  
  main().catch(console.error);
  