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
    userPositionPda: PublicKey,
    rateModel: PublicKey,
    getter: any // Enum variant object
  ): Promise<{ label: string; value0: string; value1: string; formattedValue0: number | string; formattedValue1: number | string }> {
    const sim = await program.methods
      .viewUserPositionData(getter)
      .accounts({ 
        userPosition: userPositionPda, 
        pair: pairPda,
        rateModel: rateModel
      } as any)
      .simulate();
  
    const logs = sim.raw ?? [];
    console.log(logs);

    const label = Object.keys(getter)[0]; // e.g. "userBorrowingPower"
  
    // Updated regex to match tuple format: "UserBorrowingPower: (U64(123), U64(456))"
    const match = logs
      .map((log) => log.match(new RegExp(`${label}:\\s*\\(([^,]+),\\s*([^)]+)\\)`, 'i')))
      .find(Boolean);

    // If the second value is incomplete (missing closing parenthesis), try to fix it
    if (match && match[2] && !match[2].includes(')')) {
      // Look for the complete U16/U64 pattern in the original log
      const completeMatch = logs
        .map((log) => log.match(new RegExp(`${label}:\\s*\\([^,]+,\\s*(U\\d+\\(\\d+\\))\\)`, 'i')))
        .find(Boolean);
      if (completeMatch && completeMatch[1]) {
        match[2] = completeMatch[1];
      }
    }

    console.log(match);
  
    if (!match || !match[1] || !match[2]) {
      throw new Error(`Tuple values for ${label} not found in logs`);
    }

    // Extract numeric values from OptionalUint format
    const extractValue = (optionalUintStr: string): string => {
      const valueMatch = optionalUintStr.match(/U\d+\((\d+)\)/);
      return valueMatch ? valueMatch[1] : '0';
    };

    const value0 = extractValue(match[1]);
    const value1 = extractValue(match[2]);

    const getFormattedValue = (label: string, value: string) => {
      if (label === 'userBorrowingPower') {
        return Number(value) / 10 ** 6;
      }
      if (label === 'userAppliedCollateralFactorBps' || 
          label === 'userLiquidationCollateralFactorBps' ||
          label === 'userDebtUtilizationBps') {
        return Number(value) / 100; // Convert from BPS (10000 = 100%) to decimal
      }
      if (label === 'userLiquidationPrice') {
        const numValue = Number(value);
        if (numValue === 0) {
          return 'Not applicable'; // No debt = no liquidation price
        }
        if (numValue === Number.MAX_SAFE_INTEGER || numValue >= 2**53) {
          return 'Immediately unsafe'; // u64::MAX or very large values indicate unsafe position
        }
        return Number(value) / 10 ** 6; // Liquidation price in NAD units
      }
      return Number(value) / 10 ** 6;
    };

    return { 
      label, 
      value0, 
      value1,
      formattedValue0: getFormattedValue(label, value0),
      formattedValue1: getFormattedValue(label, value1)
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

    const [userPositionPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('gamm_position'), pairPda.toBuffer(), DEPLOYER_KEYPAIR.publicKey.toBuffer()],
        program.programId
    );

    // Get and log account sizes
    const pairAccountInfo = await provider.connection.getAccountInfo(pairPda);
    const userPositionAccountInfo = await provider.connection.getAccountInfo(userPositionPda);
    
    console.log('Pair Account Size:', pairAccountInfo?.data.length ?? 0, 'bytes');
    console.log('User Position Account Size:', userPositionAccountInfo?.data.length ?? 0, 'bytes');

    console.log('User Position PDA:', userPositionPda.toBase58());
    const userPositionAccount = await program.account.userPosition.fetch(userPositionPda);
    console.log('User Collateral 0:', userPositionAccount.collateral0.toString(), Number(userPositionAccount.collateral0.toString()) / 10 ** 6);
    console.log('User Collateral 1:', Number(userPositionAccount.collateral1.toString()) / 10 ** 6);

    const pairAccount = await program.account.pair.fetch(pairPda);
    console.log('Reserve 0:', pairAccount.reserve0.toString(), Number(pairAccount.reserve0.toString()) / 10 ** 6);
    console.log('Reserve 1:', pairAccount.reserve1.toString(), Number(pairAccount.reserve1.toString()) / 10 ** 6);
    console.log('Total Debt 0:', pairAccount.totalDebt0.toString(), Number(pairAccount.totalDebt0.toString()) / 10 ** 6);
    console.log('Total Debt 1:', pairAccount.totalDebt1.toString(), Number(pairAccount.totalDebt1.toString()) / 10 ** 6);
    console.log('Rate Model:', pairAccount.rateModel.toBase58());
  
    console.log('Simulating on-chain values for user position:', userPositionPda.toBase58());
  
    // Updated enum variants to match the new UserPositionViewKind (removed functions that moved to PairViewKind)
    const enumVariants = [
      { userBorrowingPower: {} },
      { userAppliedCollateralFactorBps: {} },
      { userLiquidationCollateralFactorBps: {} },
      { userDebtUtilizationBps: {} },
      { userLiquidationPrice: {} },
    ];
  
    for (const getter of enumVariants) {
      const { label, value0, value1, formattedValue0, formattedValue1 } = await simulateGetter(program, pairPda, userPositionPda, pairAccount.rateModel, getter);
      console.log(`${label} Token0: ${value0} (${formattedValue0})`);
      console.log(`${label} Token1: ${value1} (${formattedValue1})`);
    }
  }
  
  main().catch(console.error);
  