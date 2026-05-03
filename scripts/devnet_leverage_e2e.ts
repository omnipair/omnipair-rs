import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import * as anchor from '@coral-xyz/anchor';
import { Program } from '@coral-xyz/anchor';
import BN from 'bn.js';
import {
    ASSOCIATED_TOKEN_PROGRAM_ID,
    createAssociatedTokenAccountInstruction,
    createMint,
    getAccount,
    getAssociatedTokenAddressSync,
    MINT_SIZE,
    mintTo,
    NATIVE_MINT,
    TOKEN_2022_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
} from '@solana/spl-token';
import {
    clusterApiUrl,
    Connection,
    Keypair,
    LAMPORTS_PER_SOL,
    PublicKey,
    SystemProgram,
    Transaction,
} from '@solana/web3.js';
import { PROGRAM_ID as TOKEN_METADATA_PROGRAM_ID } from '@metaplex-foundation/mpl-token-metadata/dist/src/generated/index.js';
import omnipairIdl from '../target/idl/omnipair.json' with { type: 'json' };
import leverageDelegateIdl from '../target/idl/leverage_delegate.json' with { type: 'json' };

const BPF_LOADER_UPGRADEABLE_ID = new PublicKey('BPFLoaderUpgradeab1e11111111111111111111111');
const DECIMALS = 6;
const UNIT = 10 ** DECIMALS;
const BOOTSTRAP_AMOUNT = 100_000 * UNIT;
const USER_MINT_AMOUNT = 250_000 * UNIT;
const OPEN_MARGIN = 10 * UNIT;
const OPEN_MULTIPLIER_BPS = 20_000;
const ADD_MARGIN_AMOUNT = 1 * UNIT;
const INCREASE_DEBT_AMOUNT = 2 * UNIT;
const CLOSE_PERMISSION = 1 << 0;
const ORDER_KIND_TAKE_PROFIT = 1;
const BPS_DENOMINATOR = 10_000n;
const NAD = 1_000_000_000n;

function walletPath(): string {
    return resolve(process.env.ANCHOR_WALLET ?? 'deployer-keypair.json');
}

function rpcUrl(): string {
    const configured = process.env.DEVNET_RPC_URL ?? process.env.ANCHOR_PROVIDER_URL;
    if (!configured || configured === 'devnet') {
        return clusterApiUrl('devnet');
    }
    if (configured.includes('mainnet') && process.env.DEVNET_E2E_ALLOW_MAINNET !== '1') {
        throw new Error('Refusing to run devnet e2e against a mainnet RPC URL.');
    }
    return configured;
}

function loadWallet(): Keypair {
    const path = walletPath();
    if (!existsSync(path)) {
        throw new Error(`Wallet not found at ${path}. Set ANCHOR_WALLET if needed.`);
    }
    return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(path, 'utf8'))));
}

function pda(programId: PublicKey, seeds: (Buffer | Uint8Array)[]): PublicKey {
    return PublicKey.findProgramAddressSync(seeds, programId)[0];
}

function seed(value: string): Buffer {
    return Buffer.from(value);
}

function sortMints(a: PublicKey, b: PublicKey): [PublicKey, PublicKey] {
    return a.toBuffer().compare(b.toBuffer()) < 0 ? [a, b] : [b, a];
}

function u16(value: number): Buffer {
    const buffer = Buffer.alloc(2);
    buffer.writeUInt16LE(value);
    return buffer;
}

function u64(value: number): Buffer {
    const buffer = Buffer.alloc(8);
    buffer.writeBigUInt64LE(BigInt(value));
    return buffer;
}

function toBigInt(value: BN | number | bigint): bigint {
    if (typeof value === 'bigint') {
        return value;
    }
    if (typeof value === 'number') {
        return BigInt(value);
    }
    return BigInt(value.toString());
}

function ceilDiv(numerator: bigint, denominator: bigint): bigint {
    return (numerator + denominator - 1n) / denominator;
}

function calculateAmountOut(reserveIn: bigint, reserveOut: bigint, amountIn: bigint): bigint {
    return (amountIn * reserveOut) / (reserveIn + amountIn);
}

function closeoutPricePerUnitNad(pairAccount: any, positionAccount: any) {
    const collateralAmount = toBigInt(positionAccount.collateralAmount);
    if (collateralAmount === 0n) {
        throw new Error('Cannot compute take-profit trigger for a zero-collateral position');
    }

    const isCollateralToken0 = !positionAccount.isDebtToken0;
    const reserveIn = isCollateralToken0
        ? toBigInt(pairAccount.reserve0)
        : toBigInt(pairAccount.reserve1);
    const reserveOut = isCollateralToken0
        ? toBigInt(pairAccount.reserve1)
        : toBigInt(pairAccount.reserve0);
    const swapFee = ceilDiv(
        collateralAmount * BigInt(pairAccount.swapFeeBps),
        BPS_DENOMINATOR,
    );
    const amountInAfterFee = collateralAmount - swapFee;
    const closeoutValue = calculateAmountOut(reserveIn, reserveOut, amountInAfterFee);
    return {
        closeoutValue,
        closeoutPriceNad: (closeoutValue * NAD) / collateralAmount,
    };
}

function paramsHash(args: {
    version: number;
    swapFeeBps: number;
    halfLife: number;
    fixedCfBps: number | null;
    targetUtilStartBps: number | null;
    targetUtilEndBps: number | null;
    rateHalfLifeMs: number | null;
    minRateBps: number | null;
    maxRateBps: number | null;
}): number[] {
    return Array.from(
        createHash('sha256')
            .update(Buffer.concat([
                Buffer.from([args.version]),
                u16(args.swapFeeBps),
                u64(args.halfLife),
                u16(args.fixedCfBps ?? 0),
                u64(args.targetUtilStartBps ?? 0),
                u64(args.targetUtilEndBps ?? 0),
                u64(args.rateHalfLifeMs ?? 0),
                u64(args.minRateBps ?? 0),
                u64(args.maxRateBps ?? 0),
            ]))
            .digest(),
    );
}

async function ensureLamports(connection: Connection, payer: PublicKey, minimumSol: number) {
    const balance = await connection.getBalance(payer, 'confirmed');
    if (balance >= minimumSol * LAMPORTS_PER_SOL) {
        return;
    }
    console.log(`Requesting devnet airdrop for ${payer.toBase58()}...`);
    try {
        const signature = await connection.requestAirdrop(payer, Math.ceil(minimumSol * LAMPORTS_PER_SOL));
        await connection.confirmTransaction(signature, 'confirmed');
    } catch (error) {
        const after = await connection.getBalance(payer, 'confirmed');
        if (after < minimumSol * LAMPORTS_PER_SOL) {
            throw error;
        }
        console.warn('Devnet airdrop failed, but wallet balance is already enough to continue.');
    }
}

async function ensureAta(
    provider: anchor.AnchorProvider,
    mint: PublicKey,
    owner: PublicKey,
    allowOwnerOffCurve = false,
): Promise<PublicKey> {
    const ata = getAssociatedTokenAddressSync(
        mint,
        owner,
        allowOwnerOffCurve,
        TOKEN_PROGRAM_ID,
        ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    const existing = await provider.connection.getAccountInfo(ata, 'confirmed');
    if (!existing) {
        await provider.sendAndConfirm(
            new Transaction().add(
                createAssociatedTokenAccountInstruction(
                    provider.wallet.publicKey,
                    ata,
                    owner,
                    mint,
                    TOKEN_PROGRAM_ID,
                    ASSOCIATED_TOKEN_PROGRAM_ID,
                ),
            ),
            [],
        );
    }
    return ata;
}

async function createLpMintShell(provider: anchor.AnchorProvider): Promise<Keypair> {
    const lpMint = Keypair.generate();
    const rent = await provider.connection.getMinimumBalanceForRentExemption(MINT_SIZE);
    await provider.sendAndConfirm(
        new Transaction().add(
            SystemProgram.createAccount({
                fromPubkey: provider.wallet.publicKey,
                newAccountPubkey: lpMint.publicKey,
                lamports: rent,
                space: MINT_SIZE,
                programId: TOKEN_PROGRAM_ID,
            }),
        ),
        [lpMint],
    );
    return lpMint;
}

async function maybeInitFutarchyAuthority(
    program: Program,
    provider: anchor.AnchorProvider,
    futarchyAuthority: PublicKey,
) {
    try {
        return await program.account.futarchyAuthority.fetch(futarchyAuthority);
    } catch {
        console.log('Futarchy authority not found; initializing it with the devnet deployer as authority.');
    }

    const [programData] = PublicKey.findProgramAddressSync(
        [program.programId.toBuffer()],
        BPF_LOADER_UPGRADEABLE_ID,
    );

    const signature = await program.methods
        .initFutarchyAuthority({
            authority: provider.wallet.publicKey,
            swapBps: 100,
            interestBps: 100,
            futarchyTreasury: futarchyAuthority,
            futarchyTreasuryBps: 3000,
            buybacksVault: provider.wallet.publicKey,
            buybacksVaultBps: 6000,
            teamTreasury: provider.wallet.publicKey,
            teamTreasuryBps: 1000,
        })
        .accounts({
            deployer: provider.wallet.publicKey,
            futarchyAuthority,
            programData,
            systemProgram: SystemProgram.programId,
        })
        .rpc();
    console.log('init_futarchy_authority:', signature);
    return program.account.futarchyAuthority.fetch(futarchyAuthority);
}

async function main() {
    const payer = loadWallet();
    const connection = new Connection(rpcUrl(), 'confirmed');
    const wallet = new anchor.Wallet(payer);
    const provider = new anchor.AnchorProvider(connection, wallet, {
        commitment: 'confirmed',
        preflightCommitment: 'confirmed',
        skipPreflight: false,
    });
    anchor.setProvider(provider);

    const omnipair = new Program(omnipairIdl as anchor.Idl, provider);
    const leverageDelegate = new Program(leverageDelegateIdl as anchor.Idl, provider);

    console.log('Devnet leverage e2e');
    console.log('RPC:', connection.rpcEndpoint);
    console.log('Wallet:', provider.wallet.publicKey.toBase58());
    console.log('Omnipair:', omnipair.programId.toBase58());
    console.log('Leverage delegate:', leverageDelegate.programId.toBase58());

    await ensureLamports(connection, provider.wallet.publicKey, 0.6);

    const [omnipairInfo, delegateInfo] = await Promise.all([
        connection.getAccountInfo(omnipair.programId, 'confirmed'),
        connection.getAccountInfo(leverageDelegate.programId, 'confirmed'),
    ]);
    if (!omnipairInfo || !delegateInfo) {
        throw new Error('Programs are not both deployed on this devnet RPC. Run `npm run deploy-devnet-leverage` first.');
    }

    const eventAuthority = pda(omnipair.programId, [seed('__event_authority')]);
    const futarchyAuthority = pda(omnipair.programId, [seed('futarchy_authority')]);
    const futarchy = await maybeInitFutarchyAuthority(omnipair, provider, futarchyAuthority);
    const teamTreasury = futarchy.recipients.teamTreasury as PublicKey;
    const teamTreasuryWsol = await ensureAta(provider, NATIVE_MINT, teamTreasury, true);

    const mintA = await createMint(connection, payer, provider.wallet.publicKey, null, DECIMALS);
    const mintB = await createMint(connection, payer, provider.wallet.publicKey, null, DECIMALS);
    const [token0Mint, token1Mint] = sortMints(mintA, mintB);
    const userToken0 = await ensureAta(provider, token0Mint, provider.wallet.publicKey);
    const userToken1 = await ensureAta(provider, token1Mint, provider.wallet.publicKey);
    await mintTo(connection, payer, token0Mint, userToken0, payer, BigInt(USER_MINT_AMOUNT));
    await mintTo(connection, payer, token1Mint, userToken1, payer, BigInt(USER_MINT_AMOUNT));
    console.log('Token0:', token0Mint.toBase58());
    console.log('Token1:', token1Mint.toBase58());

    const initArgs = {
        swapFeeBps: 30,
        halfLife: new BN(60_000),
        fixedCfBps: null,
        targetUtilStartBps: null,
        targetUtilEndBps: null,
        rateHalfLifeMs: null,
        minRateBps: null,
        maxRateBps: null,
        initialRateBps: null,
        paramsHash: paramsHash({
            version: 1,
            swapFeeBps: 30,
            halfLife: 60_000,
            fixedCfBps: null,
            targetUtilStartBps: null,
            targetUtilEndBps: null,
            rateHalfLifeMs: null,
            minRateBps: null,
            maxRateBps: null,
        }),
        version: 1,
        amount0In: new BN(BOOTSTRAP_AMOUNT),
        amount1In: new BN(BOOTSTRAP_AMOUNT),
        minLiquidityOut: new BN(0),
        lpName: 'Devnet Leverage omLP',
        lpSymbol: 'DVLEV-OMLP',
        lpUri: 'https://omnipair.fi/devnet/leverage-omlp.json',
    };

    const paramsHashBuffer = Buffer.from(initArgs.paramsHash);
    const pair = pda(omnipair.programId, [
        seed('gamm_pair'),
        token0Mint.toBuffer(),
        token1Mint.toBuffer(),
        paramsHashBuffer,
    ]);
    const reserve0Vault = pda(omnipair.programId, [seed('reserve_vault'), pair.toBuffer(), token0Mint.toBuffer()]);
    const reserve1Vault = pda(omnipair.programId, [seed('reserve_vault'), pair.toBuffer(), token1Mint.toBuffer()]);
    const collateral0Vault = pda(omnipair.programId, [seed('collateral_vault'), pair.toBuffer(), token0Mint.toBuffer()]);
    const collateral1Vault = pda(omnipair.programId, [seed('collateral_vault'), pair.toBuffer(), token1Mint.toBuffer()]);
    const rateModel = Keypair.generate();
    const lpMint = await createLpMintShell(provider);
    const lpMetadata = pda(TOKEN_METADATA_PROGRAM_ID, [
        seed('metadata'),
        TOKEN_METADATA_PROGRAM_ID.toBuffer(),
        lpMint.publicKey.toBuffer(),
    ]);
    const userLpAta = getAssociatedTokenAddressSync(lpMint.publicKey, provider.wallet.publicKey);

    console.log('Pair:', pair.toBase58());
    const initSignature = await omnipair.methods
        .initialize(initArgs)
        .accounts({
            deployer: provider.wallet.publicKey,
            token0Mint,
            token1Mint,
            pair,
            futarchyAuthority,
            rateModel: rateModel.publicKey,
            lpMint: lpMint.publicKey,
            lpTokenMetadata: lpMetadata,
            deployerLpTokenAccount: userLpAta,
            reserve0Vault,
            reserve1Vault,
            collateral0Vault,
            collateral1Vault,
            deployerToken0Account: userToken0,
            deployerToken1Account: userToken1,
            teamTreasury,
            teamTreasuryWsolAccount: teamTreasuryWsol,
            systemProgram: SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            tokenMetadataProgram: TOKEN_METADATA_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            rent: anchor.web3.SYSVAR_RENT_PUBKEY,
            eventAuthority,
            program: omnipair.programId,
        })
        .signers([rateModel])
        .rpc();
    console.log('initialize:', initSignature);

    const isDebtToken0 = true;
    const userLeveragePosition = pda(omnipair.programId, [
        seed('leverage_position'),
        pair.toBuffer(),
        provider.wallet.publicKey.toBuffer(),
        Buffer.from([1]),
    ]);
    const leverageCollateralVault = pda(omnipair.programId, [
        seed('leverage_collateral_vault'),
        pair.toBuffer(),
        token1Mint.toBuffer(),
    ]);

    const openSignature = await omnipair.methods
        .openLeverage({
            isDebtToken0,
            marginAmount: new BN(OPEN_MARGIN),
            multiplierBps: new BN(OPEN_MULTIPLIER_BPS),
            minCollateralOut: new BN(0),
        })
        .accounts({
            pair,
            rateModel: rateModel.publicKey,
            futarchyAuthority,
            userLeveragePosition,
            tokenInVault: reserve0Vault,
            tokenOutVault: reserve1Vault,
            leverageCollateralVault,
            userTokenInAccount: userToken0,
            tokenInMint: token0Mint,
            tokenOutMint: token1Mint,
            user: provider.wallet.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            eventAuthority,
            program: omnipair.programId,
        })
        .rpc();
    console.log('open_leverage:', openSignature);

    let position = await omnipair.account.userLeveragePosition.fetch(userLeveragePosition);
    if (position.debtShares.isZero?.() ? true : position.debtShares.eq(new BN(0))) {
        throw new Error('open_leverage did not create debt shares');
    }
    if (position.collateralAmount.toNumber() <= 0) {
        throw new Error('open_leverage did not escrow collateral');
    }
    console.log('Opened collateral:', position.collateralAmount.toString(), 'debt shares:', position.debtShares.toString());

    const addMarginSignature = await omnipair.methods
        .addLeverageMargin({
            isDebtToken0,
            amount: new BN(ADD_MARGIN_AMOUNT),
        })
        .accounts({
            pair,
            rateModel: rateModel.publicKey,
            futarchyAuthority,
            positionOwner: provider.wallet.publicKey,
            userLeveragePosition,
            debtTokenVault: reserve0Vault,
            sourceTokenAccount: userToken0,
            debtTokenMint: token0Mint,
            userLeverageDelegation: null,
            delegatedProgram: null,
            authority: provider.wallet.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            eventAuthority,
            program: omnipair.programId,
        })
        .rpc();
    console.log('add_leverage_margin:', addMarginSignature);

    const increaseSignature = await omnipair.methods
        .increaseLeverage({
            isDebtToken0,
            debtAmount: new BN(INCREASE_DEBT_AMOUNT),
            minCollateralOut: new BN(0),
        })
        .accounts({
            pair,
            rateModel: rateModel.publicKey,
            futarchyAuthority,
            positionOwner: provider.wallet.publicKey,
            userLeveragePosition,
            debtTokenVault: reserve0Vault,
            collateralTokenVault: reserve1Vault,
            leverageCollateralVault,
            debtTokenMint: token0Mint,
            collateralTokenMint: token1Mint,
            userLeverageDelegation: null,
            delegatedProgram: null,
            authority: provider.wallet.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            eventAuthority,
            program: omnipair.programId,
        })
        .rpc();
    console.log('increase_leverage:', increaseSignature);

    position = await omnipair.account.userLeveragePosition.fetch(userLeveragePosition);
    const decreaseAmount = Math.max(1, Math.floor(position.collateralAmount.toNumber() / 10));
    const decreaseSignature = await omnipair.methods
        .decreaseLeverage({
            isDebtToken0,
            collateralAmount: new BN(decreaseAmount),
            minAmountOut: new BN(0),
        })
        .accounts({
            pair,
            rateModel: rateModel.publicKey,
            futarchyAuthority,
            positionOwner: provider.wallet.publicKey,
            userLeveragePosition,
            collateralTokenVault: reserve1Vault,
            debtTokenVault: reserve0Vault,
            leverageCollateralVault,
            collateralTokenMint: token1Mint,
            debtTokenMint: token0Mint,
            userLeverageDelegation: null,
            delegatedProgram: null,
            authority: provider.wallet.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            eventAuthority,
            program: omnipair.programId,
        })
        .rpc();
    console.log('decrease_leverage:', decreaseSignature);

    const userLeverageDelegation = pda(omnipair.programId, [
        seed('leverage_delegation'),
        userLeveragePosition.toBuffer(),
    ]);
    const delegationSignature = await omnipair.methods
        .createLeverageDelegation({
            isDebtToken0,
            delegatedProgram: leverageDelegate.programId,
            approvedActions: CLOSE_PERMISSION,
        })
        .accounts({
            pair,
            userLeveragePosition,
            userLeverageDelegation,
            owner: provider.wallet.publicKey,
            systemProgram: SystemProgram.programId,
        })
        .rpc();
    console.log('create_leverage_delegation:', delegationSignature);

    const orderId = Date.now();
    const order = pda(leverageDelegate.programId, [
        seed('leverage_order'),
        userLeveragePosition.toBuffer(),
        provider.wallet.publicKey.toBuffer(),
        u64(orderId),
    ]);
    const pairForTp = await omnipair.account.pair.fetch(pair);
    position = await omnipair.account.userLeveragePosition.fetch(userLeveragePosition);
    const takeProfit = closeoutPricePerUnitNad(pairForTp, position);
    console.log(
        'take_profit_trigger_closeout_price_nad:',
        takeProfit.closeoutPriceNad.toString(),
        'closeout_value:',
        takeProfit.closeoutValue.toString(),
    );
    const createOrderSignature = await leverageDelegate.methods
        .createLeverageOrder({
            orderId: new BN(orderId),
            kind: ORDER_KIND_TAKE_PROFIT,
            triggerCloseoutPriceNad: new BN(takeProfit.closeoutPriceNad.toString()),
        })
        .accounts({
            pair,
            userLeveragePosition,
            order,
            owner: provider.wallet.publicKey,
            systemProgram: SystemProgram.programId,
        })
        .rpc();
    console.log('create_leverage_order:', createOrderSignature);

    const custodyAuthority = pda(leverageDelegate.programId, [
        seed('leverage_delegate_authority'),
        order.toBuffer(),
    ]);
    const executor = Keypair.generate();
    const custodyTokenAccount = await ensureAta(provider, token0Mint, custodyAuthority, true);
    const executorTokenAccount = await ensureAta(provider, token0Mint, executor.publicKey);
    const ownerTokenAccount = userToken0;
    console.log('keeper_executor:', executor.publicKey.toBase58());

    const beforeIx = await leverageDelegate.methods
        .beforeTakeProfit({ orderId: new BN(orderId) })
        .accounts({
            order,
            pair,
            userLeveragePosition,
            userLeverageDelegation,
            custodyAuthority,
            custodyTokenAccount,
            tokenMint: token0Mint,
            executor: executor.publicKey,
        })
        .instruction();

    const afterIx = await leverageDelegate.methods
        .afterCloseOrder({ orderId: new BN(orderId) })
        .accounts({
            order,
            owner: provider.wallet.publicKey,
            userLeveragePosition,
            custodyAuthority,
            custodyTokenAccount,
            executorTokenAccount,
            ownerTokenAccount,
            tokenMint: token0Mint,
            executor: executor.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
        })
        .instruction();

    const beforeAccounts = [
        { pubkey: order, isWritable: true, isSigner: false },
        { pubkey: pair, isWritable: false, isSigner: false },
        { pubkey: userLeveragePosition, isWritable: false, isSigner: false },
        { pubkey: userLeverageDelegation, isWritable: false, isSigner: false },
        { pubkey: custodyAuthority, isWritable: false, isSigner: false },
        { pubkey: custodyTokenAccount, isWritable: false, isSigner: false },
        { pubkey: token0Mint, isWritable: false, isSigner: false },
        { pubkey: executor.publicKey, isWritable: false, isSigner: true },
    ];
    const afterAccounts = [
        { pubkey: order, isWritable: true, isSigner: false },
        { pubkey: provider.wallet.publicKey, isWritable: true, isSigner: false },
        { pubkey: userLeveragePosition, isWritable: false, isSigner: false },
        { pubkey: custodyAuthority, isWritable: false, isSigner: false },
        { pubkey: custodyTokenAccount, isWritable: true, isSigner: false },
        { pubkey: executorTokenAccount, isWritable: true, isSigner: false },
        { pubkey: ownerTokenAccount, isWritable: true, isSigner: false },
        { pubkey: token0Mint, isWritable: false, isSigner: false },
        { pubkey: executor.publicKey, isWritable: false, isSigner: true },
        { pubkey: TOKEN_PROGRAM_ID, isWritable: false, isSigner: false },
    ];

    const delegatedCloseSignature = await omnipair.methods
        .delegatedCloseLeverage({
            isDebtToken0,
            minAmountOut: new BN(0),
            delegated: {
                beforeIxData: beforeIx.data,
                afterIxData: afterIx.data,
                beforeAccountsLen: beforeAccounts.length,
            },
        })
        .accounts({
            pair,
            rateModel: rateModel.publicKey,
            futarchyAuthority,
            positionOwner: provider.wallet.publicKey,
            userLeveragePosition,
            tokenInVault: reserve1Vault,
            tokenOutVault: reserve0Vault,
            leverageCollateralVault,
            recipientTokenOutAccount: custodyTokenAccount,
            tokenInMint: token1Mint,
            tokenOutMint: token0Mint,
            userLeverageDelegation,
            delegatedProgram: leverageDelegate.programId,
            authority: executor.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            eventAuthority,
            program: omnipair.programId,
        })
        .remainingAccounts([...beforeAccounts, ...afterAccounts])
        .signers([executor])
        .rpc();
    console.log('delegated_close_leverage:', delegatedCloseSignature);

    const closedPosition = await connection.getAccountInfo(userLeveragePosition, 'confirmed');
    if (closedPosition) {
        throw new Error('delegated close did not close the leverage position account');
    }
    const closedOrder = await connection.getAccountInfo(order, 'confirmed');
    if (closedOrder) {
        throw new Error('after_close_order did not close the delegate order account');
    }
    const custodyBalance = Number((await getAccount(connection, custodyTokenAccount)).amount);
    if (custodyBalance !== 0) {
        throw new Error(`delegate custody should be empty after settlement, found ${custodyBalance}`);
    }

    console.log('\nDevnet leverage e2e passed.');
    console.log('Pair:', pair.toBase58());
    console.log('Position closed:', userLeveragePosition.toBase58());
}

main().catch((error) => {
    console.error(error);
    process.exit(1);
});
