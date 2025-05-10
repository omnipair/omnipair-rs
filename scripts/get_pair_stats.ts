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
  ): Promise<{ label: string; value: string; formattedValue: number }> {
    const sim = await program.methods
      .viewPairData(getter)
      .accounts({ pair: pairPda })
      .simulate();
  
    const logs = sim.raw ?? [];
    console.log(logs);

    const label = Object.keys(getter)[0]; // e.g. "price0Nad"
  
    const match = logs
      .map((log) => log.match(new RegExp(`${label}:\\s*(\\d+)`, 'i')))
      .find(Boolean);

    console.log(match);
  
    if (!match || !match[1]) {
      throw new Error(`Value for ${label} not found in logs`);
    }
    const getFormattedValue = (label: string, value: string) => {
      if (label === 'userToken0BorrowingPower' || label === 'userToken1BorrowingPower') {
        return Number(value) / 10 ** 9;
      }
      if (label === 'userToken0EffectiveCollateralFactorBps' || label === 'userToken1EffectiveCollateralFactorBps') {
        return Number(value) / 100; // Convert from BPS (10000 = 100%) to decimal
      }
      if (label === 'spotPrice0Nad' || label === 'spotPrice1Nad') {
        return Number(value) / 10 ** 9;
      }
      if (label === 'emaPrice0Nad' || label === 'emaPrice1Nad') {
        return Number(value) / 10 ** 9;
      }
      return Number(value) / 10 ** 6;
    };

    return { 
      label, 
      value: match[1], 
      formattedValue: getFormattedValue(label, match[1])
    };
  }
  
  async function main() {
    const provider = anchor.AnchorProvider.env();
    const program = new Program<Omnipair>(idl, provider);
    const DEPLOYER_KEYPAIR = provider.wallet.payer;

    if(!DEPLOYER_KEYPAIR) {
        throw new Error('Deployer keypair not found');
    }
  
    const [pairPda] = PublicKey.findProgramAddressSync(
      [Buffer.from('gamm_pair'), TOKEN0_MINT.toBuffer(), TOKEN1_MINT.toBuffer()],
      program.programId
    );

    const pairAccount = await program.account.pair.fetch(pairPda);
    console.log('Reserve 0:', pairAccount.reserve0.toString(), Number(pairAccount.reserve0.toString()) / 10 ** 6);
    console.log('Reserve 1:', pairAccount.reserve1.toString(), Number(pairAccount.reserve1.toString()) / 10 ** 6);
    console.log('Total Debt 0:', pairAccount.totalDebt0.toString(), Number(pairAccount.totalDebt0.toString()) / 10 ** 6);
    console.log('Total Debt 1:', pairAccount.totalDebt1.toString(), Number(pairAccount.totalDebt1.toString()) / 10 ** 6);
  
    console.log('Simulating on-chain prices for pair:', pairPda.toBase58());
  
    // notice its written camelCase not PascalCase
    // although PascalCase is used in the idl and the value returned in logs is PascalCase
    // it will through an error if you use PascalCase
    const enumVariants = [
      { emaPrice0Nad: {} },
      { emaPrice1Nad: {} },
      { spotPrice0Nad: {} },
      { spotPrice1Nad: {} }
    ];
  
    for (const getter of enumVariants) {
      const { label, value, formattedValue } = await simulateGetter(program, pairPda, getter);
      console.log(`${label}: ${value} (${formattedValue})`);
    }
  }
  
  main().catch(console.error);
  