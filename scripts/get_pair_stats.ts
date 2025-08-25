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
  import BN from 'bn.js';
  
  dotenv.config();
  
  // === Load environment ===
  const TOKEN0_MINT = new PublicKey(process.env.TOKEN0_MINT || '');
  const TOKEN1_MINT = new PublicKey(process.env.TOKEN1_MINT || '');
  
  async function simulateGetter(
    program: Program<Omnipair>,
    pairPda: PublicKey,
    getter: any, // Enum variant object
    args?: any, // EmitValueArgs for functions that need additional parameters
    rateModelPda?: PublicKey // Rate model PDA for getRates function
  ): Promise<{ label: string; value0: string; value1: string; formattedValue0: number; formattedValue1: number }> {
    const accounts: any = { pair: pairPda };
    
    // Add rate model account - required for all ViewPairData functions
    if (rateModelPda) {
      accounts.rateModel = rateModelPda;
    }
    
    const sim = await program.methods
      .viewPairData(getter, args || { debtAmount: null, collateralAmount: null, collateralToken: null })
      .accounts(accounts)
      .simulate();
  
    const logs = sim.raw ?? [];
    console.log(logs);

    const label = Object.keys(getter)[0]; // e.g. "emaPrice0Nad"
  
    // Updated regex to match tuple format: "EmaPrice0Nad: (U64(123), OptionalU64(None))"
    // Use a more specific pattern to capture the complete second value
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
  
    if (!match || !match[1] || !match[2]) {
      throw new Error(`Tuple values for ${label} not found in logs`);
    }

    // Extract numeric values from OptionalUint format
    const extractValue = (optionalUintStr: string): string => {
      // Handle both U64 and U16 formats
      const valueMatch = optionalUintStr.match(/U\d+\((\d+)\)/);
      return valueMatch ? valueMatch[1] : '0';
    };

    const value0 = extractValue(match[1]);
    const value1 = extractValue(match[2]);

    const getFormattedValue = (label: string, value: string, isSecondValue: boolean = false) => {
      if (label === 'emaPrice0Nad' || label === 'emaPrice1Nad' || 
          label === 'spotPrice0Nad' || label === 'spotPrice1Nad') {
        return Number(value) / 10 ** 9;
      }
      if (label === 'k') {
        return Number(value) / 10 ** 12;
      }
      if (label === 'getRates') {
        return Number(value) / 10 ** 9; // Rates are in NAD format (1e9)
      }
      if (label === 'getMinCollateralForDebt') {
        return Number(value) / 10 ** 6;
      }
      if (label === 'getBorrowLimitAndCfBpsForCollateral') {
        // For getBorrowLimitAndCfBpsForCollateral, the second value (CF BPS) should not be divided
        if (isSecondValue) {
          return Number(value); // CF BPS is already in basis points, no division needed
        }
        return Number(value) / 10 ** 6; // First value (borrow limit) should be divided
      }
      return Number(value) / 10 ** 6;
    };

    return { 
      label, 
      value0, 
      value1,
      formattedValue0: getFormattedValue(label, value0),
      formattedValue1: getFormattedValue(label, value1, true)
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
    console.log('Rate Model:', pairAccount.rateModel.toBase58());
  
    console.log('Simulating on-chain values for pair:', pairPda.toBase58());
  
    // Updated enum variants to match the new PairViewKind
    const enumVariants = [
      { emaPrice0Nad: {} },
      { emaPrice1Nad: {} },
      { spotPrice0Nad: {} },
      { spotPrice1Nad: {} },
      { k: {} },
      { getRates: {} }
    ];
  
    for (const getter of enumVariants) {
      const { label, value0, value1, formattedValue0, formattedValue1 } = await simulateGetter(program, pairPda, getter, undefined, pairAccount.rateModel);
      console.log(`${label}: ${value0} (${formattedValue0})${label === 'getRates' ? `, ${value1} (${formattedValue1})` : ''}`);
      // Note: value1 is OptionalU64(None) for single-value functions, so we don't display it
    }

    // Test GetMinCollateralForDebt with a sample debt amount (1 token)
    const debtAmount = new BN(1_000_000); // 1 token in lamports
    console.log(`\nTesting GetMinCollateralForDebt with debt amount: ${debtAmount.toNumber() / 10 ** 6} tokens`);
    const minCollateralResult = await simulateGetter(
      program, 
      pairPda, 
      { getMinCollateralForDebt: {} },
      { debtAmount: debtAmount, collateralAmount: null, collateralToken: null },
      pairAccount.rateModel
    );
    console.log(`${minCollateralResult.label} Token0: ${minCollateralResult.value0} (${minCollateralResult.formattedValue0})`);
    console.log(`${minCollateralResult.label} Token1: ${minCollateralResult.value1} (${minCollateralResult.formattedValue1})`);

    // Test GetBorrowLimitAndCfBpsForCollateral with 20% of collateral reserve
    const collateralReserve0 = Number(pairAccount.reserve0.toString());
    const collateralReserve1 = Number(pairAccount.reserve1.toString());
    const testCollateralAmount0 = new BN(Math.floor(collateralReserve0 * 0.05)); // 5% of reserve
    const testCollateralAmount1 = new BN(Math.floor(collateralReserve1 * 0.05)); // 5% of reserve

    console.log(`\nTesting GetBorrowLimitAndCfBpsForCollateral with 20% of collateral reserves:`);
    console.log(`Test collateral amount Token0: ${testCollateralAmount0.toNumber() / 10 ** 6} tokens`);
    console.log(`Test collateral amount Token1: ${testCollateralAmount1.toNumber() / 10 ** 6} tokens`);

    // Test with Token0 as collateral
    const borrowLimitResult0 = await simulateGetter(
      program, 
      pairPda, 
      { getBorrowLimitAndCfBpsForCollateral: {} },
      { debtAmount: null, collateralAmount: testCollateralAmount0, collateralToken: TOKEN0_MINT },
      pairAccount.rateModel
    );
    console.log(borrowLimitResult0);
    console.log(`${borrowLimitResult0.label} with Token0 collateral - Max Debt: ${borrowLimitResult0.value0} (${borrowLimitResult0.formattedValue0}), CF BPS: ${borrowLimitResult0.value1} (${borrowLimitResult0.formattedValue1 / 100}%)`);

    // Test with Token1 as collateral
    const borrowLimitResult1 = await simulateGetter(
      program, 
      pairPda, 
      { getBorrowLimitAndCfBpsForCollateral: {} },
      { debtAmount: null, collateralAmount: testCollateralAmount1, collateralToken: TOKEN1_MINT },
      pairAccount.rateModel
    );
    console.log(`${borrowLimitResult1.label} with Token1 collateral - Max Debt: ${borrowLimitResult1.value0} (${borrowLimitResult1.formattedValue0}), CF BPS: ${borrowLimitResult1.value1} (${borrowLimitResult1.formattedValue1 / 100}%)`);
  }
  
  main().catch(console.error);
  