import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import * as anchor from '@coral-xyz/anchor';
import { Program } from '@coral-xyz/anchor';
import BN from 'bn.js';
import {
    ASSOCIATED_TOKEN_PROGRAM_ID,
    createAssociatedTokenAccountInstruction,
    getAccount,
    getAssociatedTokenAddressSync,
    getMint,
    TOKEN_2022_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
} from '@solana/spl-token';
import {
    Connection,
    Keypair,
    LAMPORTS_PER_SOL,
    PublicKey,
    SystemProgram,
    Transaction,
} from '@solana/web3.js';
import omnipairIdl from '../target/idl/omnipair.json' with { type: 'json' };
import leverageDelegateIdl from '../target/idl/leverage_delegate.json' with { type: 'json' };

const SURFPOOL_URL = process.env.SURFPOOL_RPC_URL ?? process.env.ANCHOR_PROVIDER_URL ?? 'http://127.0.0.1:8899';
const SELECTED_PAIR = new PublicKey(
    process.env.SURFPOOL_LEVERAGE_PAIR ?? 'Cp2nGCWWfqkUmPR3pPKoR376Fti8wuYRFrSWJZq1a9SA',
);
const EXPECTED_TOKEN0 = new PublicKey('METAwkXcqyXKy1AtsSgJ8JiUHwGCafnZL38n3vYmeta');
const EXPECTED_TOKEN1 = new PublicKey('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v');
const EXPECTED_RATE_MODEL = new PublicKey('sABETiFEiSHGbvYL6kWqz5r8RnkhsnV5Eewb5x6ppbb');
const CLOSE_PERMISSION = 1 << 0;
const ORDER_KIND_TAKE_PROFIT = 1;
const BPS_DENOMINATOR = 10_000n;
const NAD = 1_000_000_000n;
const OPEN_MULTIPLIER_BPS = 20_000;

type PairAccount = {
    token0: PublicKey;
    token1: PublicKey;
    rateModel: PublicKey;
    reserve0: BN;
    reserve1: BN;
    cashReserve0: BN;
    cashReserve1: BN;
    swapFeeBps: number;
    reduceOnly: boolean;
};

function walletPath(): string {
    return resolve(process.env.ANCHOR_WALLET ?? 'deployer-keypair.json');
}

function loadWallet(): Keypair {
    const path = walletPath();
    if (!existsSync(path)) {
        throw new Error(`Wallet not found at ${path}. Set ANCHOR_WALLET if needed.`);
    }
    return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(path, 'utf8'))));
}

function seed(value: string): Buffer {
    return Buffer.from(value);
}

function pda(programId: PublicKey, seeds: (Buffer | Uint8Array)[]): PublicKey {
    return PublicKey.findProgramAddressSync(seeds, programId)[0];
}

function u64(value: number): Buffer {
    const buffer = Buffer.alloc(8);
    buffer.writeBigUInt64LE(BigInt(value));
    return buffer;
}

function toBN(value: bigint): BN {
    return new BN(value.toString());
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

function closeoutPricePerUnitNad(pair: PairAccount, position: any) {
    const collateralAmount = toBigInt(position.collateralAmount);
    if (collateralAmount === 0n) {
        throw new Error('Cannot compute take-profit trigger for a zero-collateral position');
    }

    const isCollateralToken0 = !position.isDebtToken0;
    const reserveIn = isCollateralToken0 ? toBigInt(pair.reserve0) : toBigInt(pair.reserve1);
    const reserveOut = isCollateralToken0 ? toBigInt(pair.reserve1) : toBigInt(pair.reserve0);
    const swapFee = ceilDiv(collateralAmount * BigInt(pair.swapFeeBps), BPS_DENOMINATOR);
    const amountInAfterFee = collateralAmount - swapFee;
    const closeoutValue = calculateAmountOut(reserveIn, reserveOut, amountInAfterFee);
    return {
        closeoutValue,
        closeoutPriceNad: (closeoutValue * NAD) / collateralAmount,
    };
}

async function rpcRequest(connection: Connection, method: string, params: unknown[]) {
    const response = await fetch(connection.rpcEndpoint, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
            jsonrpc: '2.0',
            id: 1,
            method,
            params,
        }),
    });
    const payload = await response.json() as { result?: unknown; error?: unknown };
    if (payload.error) {
        throw new Error(`${method} failed: ${JSON.stringify(payload.error)}`);
    }
    return payload.result;
}

async function waitForSurfpool(connection: Connection) {
    for (let attempt = 0; attempt < 30; attempt += 1) {
        try {
            await connection.getVersion();
            return;
        } catch {
            await new Promise((resolve) => setTimeout(resolve, 1_000));
        }
    }
    throw new Error(`Surfpool RPC is not reachable at ${connection.rpcEndpoint}`);
}

async function setLamports(connection: Connection, pubkey: PublicKey, sol: number) {
    try {
        const signature = await connection.requestAirdrop(pubkey, sol * LAMPORTS_PER_SOL);
        await connection.confirmTransaction(signature, 'confirmed');
        return;
    } catch {
        await rpcRequest(connection, 'surfnet_setAccount', [
            pubkey.toBase58(),
            {
                lamports: sol * LAMPORTS_PER_SOL,
                owner: SystemProgram.programId.toBase58(),
            },
        ]);
    }
}

async function setTokenBalance(
    connection: Connection,
    owner: PublicKey,
    mint: PublicKey,
    amount: bigint,
) {
    if (amount > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new Error(`surfnet_setTokenAccount amount is above JSON safe integer range: ${amount}`);
    }
    await rpcRequest(connection, 'surfnet_setTokenAccount', [
        owner.toBase58(),
        mint.toBase58(),
        {
            amount: Number(amount),
            state: 'initialized',
        },
        TOKEN_PROGRAM_ID.toBase58(),
    ]);
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

async function assertProgramReadable(connection: Connection, programId: PublicKey, label: string) {
    const info = await connection.getAccountInfo(programId, 'confirmed');
    if (!info || !info.executable) {
        throw new Error(`${label} program is not deployed on the Surfpool fork: ${programId.toBase58()}`);
    }
}

async function assertLocalProgramBytes(
    connection: Connection,
    programId: PublicKey,
    soPath: string,
    label: string,
) {
    const programInfo = await connection.getAccountInfo(programId, 'confirmed');
    if (!programInfo || !programInfo.executable) {
        throw new Error(`${label} program is not executable on the Surfpool fork: ${programId.toBase58()}`);
    }
    if (programInfo.data.length < 36 || programInfo.data.readUInt32LE(0) !== 2) {
        throw new Error(`${label} is not an upgradeable-loader program account`);
    }

    const programData = new PublicKey(programInfo.data.subarray(4, 36));
    const programDataInfo = await connection.getAccountInfo(programData, 'confirmed');
    if (!programDataInfo) {
        throw new Error(`${label} programdata is not readable: ${programData.toBase58()}`);
    }

    const localBytes = readFileSync(resolve(soPath));
    let elfOffset = -1;
    for (let index = 0; index + 4 <= programDataInfo.data.length; index += 1) {
        if (
            programDataInfo.data[index] === 0x7f
            && programDataInfo.data[index + 1] === 0x45
            && programDataInfo.data[index + 2] === 0x4c
            && programDataInfo.data[index + 3] === 0x46
        ) {
            elfOffset = index;
            break;
        }
    }
    if (elfOffset === -1) {
        throw new Error(`${label} programdata does not contain an ELF payload`);
    }
    const deployedBytes = programDataInfo.data.subarray(elfOffset, elfOffset + localBytes.length);
    if (deployedBytes.length !== localBytes.length || !Buffer.from(deployedBytes).equals(localBytes)) {
        throw new Error(
            `${label} on Surfpool does not match ${soPath}; restart Surfpool with local deployment enabled`,
        );
    }
}

function amountForReserve(reserve: BN) {
    const reserveRaw = toBigInt(reserve);
    const margin = reserveRaw / 1_000n;
    if (margin === 0n) {
        throw new Error('Selected pair reserve is too small for the default leverage smoke test');
    }
    return {
        margin,
        addMargin: margin / 10n || 1n,
        increaseDebt: margin / 5n || 1n,
        ownerFunding: margin * 100n,
    };
}

async function main() {
    if (!SURFPOOL_URL.includes('127.0.0.1') && !SURFPOOL_URL.includes('localhost')) {
        throw new Error(`Refusing to run Surfpool e2e against non-local RPC: ${SURFPOOL_URL}`);
    }

    const payer = loadWallet();
    const connection = new Connection(SURFPOOL_URL, 'confirmed');
    const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(payer), {
        commitment: 'confirmed',
        preflightCommitment: 'confirmed',
        skipPreflight: false,
    });
    anchor.setProvider(provider);

    await waitForSurfpool(connection);

    const omnipair = new Program(omnipairIdl as anchor.Idl, provider);
    const leverageDelegate = new Program(leverageDelegateIdl as anchor.Idl, provider);
    const owner = Keypair.generate();
    const executor = Keypair.generate();

    console.log('Surfpool mainnet-fork leverage e2e');
    console.log('RPC:', connection.rpcEndpoint);
    console.log('Pair:', SELECTED_PAIR.toBase58());
    console.log('Owner:', owner.publicKey.toBase58());
    console.log('Executor:', executor.publicKey.toBase58());

    await assertProgramReadable(connection, omnipair.programId, 'Omnipair');
    await assertProgramReadable(connection, leverageDelegate.programId, 'Leverage delegate');
    await assertLocalProgramBytes(connection, omnipair.programId, 'target/deploy/omnipair.so', 'Omnipair');
    await assertLocalProgramBytes(
        connection,
        leverageDelegate.programId,
        'target/deploy/leverage_delegate.so',
        'Leverage delegate',
    );
    await Promise.all([
        setLamports(connection, provider.wallet.publicKey, 100),
        setLamports(connection, owner.publicKey, 100),
        setLamports(connection, executor.publicKey, 10),
    ]);

    const pairInfo = await connection.getAccountInfo(SELECTED_PAIR, 'confirmed');
    if (!pairInfo || !pairInfo.owner.equals(omnipair.programId)) {
        throw new Error('Selected pair is not a readable Omnipair pair on this fork');
    }
    let pair = await omnipair.account.pair.fetch(SELECTED_PAIR) as PairAccount;
    if (!pair.token0.equals(EXPECTED_TOKEN0) || !pair.token1.equals(EXPECTED_TOKEN1)) {
        throw new Error('Selected pair token mints do not match the expected mainnet pair');
    }
    if (!pair.rateModel.equals(EXPECTED_RATE_MODEL)) {
        throw new Error('Selected pair rate model does not match the expected mainnet pair');
    }
    if (pair.reduceOnly) {
        throw new Error('Selected pair is reduce-only; cannot open leverage');
    }

    const token0MintInfo = await getMint(connection, pair.token0);
    const token1MintInfo = await getMint(connection, pair.token1);
    const amounts = amountForReserve(pair.reserve0);
    await Promise.all([
        setTokenBalance(connection, owner.publicKey, pair.token0, amounts.ownerFunding),
        setTokenBalance(connection, owner.publicKey, pair.token1, 100_000n * (10n ** BigInt(token1MintInfo.decimals))),
    ]);

    const userToken0 = getAssociatedTokenAddressSync(pair.token0, owner.publicKey);
    const userToken1 = getAssociatedTokenAddressSync(pair.token1, owner.publicKey);
    const userToken0Account = await getAccount(connection, userToken0);
    const userToken1Account = await getAccount(connection, userToken1);
    if (userToken0Account.amount < amounts.ownerFunding || userToken1Account.amount === 0n) {
        throw new Error('surfnet_setTokenAccount did not fund owner token accounts');
    }

    const eventAuthority = pda(omnipair.programId, [seed('__event_authority')]);
    const futarchyAuthority = pda(omnipair.programId, [seed('futarchy_authority')]);
    await omnipair.account.futarchyAuthority.fetch(futarchyAuthority);

    const reserve0Vault = pda(omnipair.programId, [seed('reserve_vault'), SELECTED_PAIR.toBuffer(), pair.token0.toBuffer()]);
    const reserve1Vault = pda(omnipair.programId, [seed('reserve_vault'), SELECTED_PAIR.toBuffer(), pair.token1.toBuffer()]);
    await Promise.all([
        getAccount(connection, reserve0Vault),
        getAccount(connection, reserve1Vault),
        connection.getAccountInfo(pair.rateModel, 'confirmed'),
    ]);

    const isDebtToken0 = true;
    const userLeveragePosition = pda(omnipair.programId, [
        seed('leverage_position'),
        SELECTED_PAIR.toBuffer(),
        owner.publicKey.toBuffer(),
        Buffer.from([1]),
    ]);
    const leverageCollateralVault = pda(omnipair.programId, [
        seed('leverage_collateral_vault'),
        SELECTED_PAIR.toBuffer(),
        pair.token1.toBuffer(),
    ]);

    console.log('open_margin_raw:', amounts.margin.toString());
    const openSignature = await omnipair.methods
        .openLeverage({
            isDebtToken0,
            marginAmount: toBN(amounts.margin),
            multiplierBps: new BN(OPEN_MULTIPLIER_BPS),
            minCollateralOut: new BN(0),
        })
        .accounts({
            pair: SELECTED_PAIR,
            rateModel: pair.rateModel,
            futarchyAuthority,
            userLeveragePosition,
            tokenInVault: reserve0Vault,
            tokenOutVault: reserve1Vault,
            leverageCollateralVault,
            userTokenInAccount: userToken0,
            tokenInMint: pair.token0,
            tokenOutMint: pair.token1,
            user: owner.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            eventAuthority,
            program: omnipair.programId,
        })
        .signers([owner])
        .rpc();
    console.log('open_leverage:', openSignature);

    let position = await omnipair.account.userLeveragePosition.fetch(userLeveragePosition);
    if (toBigInt(position.debtShares) === 0n || toBigInt(position.collateralAmount) === 0n) {
        throw new Error('open_leverage did not create debt/collateral state');
    }

    const addMarginSignature = await omnipair.methods
        .addLeverageMargin({
            isDebtToken0,
            amount: toBN(amounts.addMargin),
        })
        .accounts({
            pair: SELECTED_PAIR,
            rateModel: pair.rateModel,
            futarchyAuthority,
            positionOwner: owner.publicKey,
            userLeveragePosition,
            debtTokenVault: reserve0Vault,
            sourceTokenAccount: userToken0,
            debtTokenMint: pair.token0,
            userLeverageDelegation: null,
            delegatedProgram: null,
            authority: owner.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            eventAuthority,
            program: omnipair.programId,
        })
        .signers([owner])
        .rpc();
    console.log('add_leverage_margin:', addMarginSignature);

    const increaseSignature = await omnipair.methods
        .increaseLeverage({
            isDebtToken0,
            debtAmount: toBN(amounts.increaseDebt),
            minCollateralOut: new BN(0),
        })
        .accounts({
            pair: SELECTED_PAIR,
            rateModel: pair.rateModel,
            futarchyAuthority,
            positionOwner: owner.publicKey,
            userLeveragePosition,
            debtTokenVault: reserve0Vault,
            collateralTokenVault: reserve1Vault,
            leverageCollateralVault,
            debtTokenMint: pair.token0,
            collateralTokenMint: pair.token1,
            userLeverageDelegation: null,
            delegatedProgram: null,
            authority: owner.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            eventAuthority,
            program: omnipair.programId,
        })
        .signers([owner])
        .rpc();
    console.log('increase_leverage:', increaseSignature);

    position = await omnipair.account.userLeveragePosition.fetch(userLeveragePosition);
    const decreaseAmount = toBigInt(position.collateralAmount) / 10n || 1n;
    const decreaseSignature = await omnipair.methods
        .decreaseLeverage({
            isDebtToken0,
            collateralAmount: toBN(decreaseAmount),
            minAmountOut: new BN(0),
        })
        .accounts({
            pair: SELECTED_PAIR,
            rateModel: pair.rateModel,
            futarchyAuthority,
            positionOwner: owner.publicKey,
            userLeveragePosition,
            collateralTokenVault: reserve1Vault,
            debtTokenVault: reserve0Vault,
            leverageCollateralVault,
            collateralTokenMint: pair.token1,
            debtTokenMint: pair.token0,
            userLeverageDelegation: null,
            delegatedProgram: null,
            authority: owner.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            eventAuthority,
            program: omnipair.programId,
        })
        .signers([owner])
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
            pair: SELECTED_PAIR,
            userLeveragePosition,
            userLeverageDelegation,
            owner: owner.publicKey,
            systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();
    console.log('create_leverage_delegation:', delegationSignature);

    const orderId = Date.now();
    const order = pda(leverageDelegate.programId, [
        seed('leverage_order'),
        userLeveragePosition.toBuffer(),
        owner.publicKey.toBuffer(),
        u64(orderId),
    ]);
    pair = await omnipair.account.pair.fetch(SELECTED_PAIR) as PairAccount;
    position = await omnipair.account.userLeveragePosition.fetch(userLeveragePosition);
    const takeProfit = closeoutPricePerUnitNad(pair, position);
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
            triggerCloseoutPriceNad: toBN(takeProfit.closeoutPriceNad),
        })
        .accounts({
            pair: SELECTED_PAIR,
            userLeveragePosition,
            order,
            owner: owner.publicKey,
            systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();
    console.log('create_leverage_order:', createOrderSignature);

    const custodyAuthority = pda(leverageDelegate.programId, [
        seed('leverage_delegate_authority'),
        order.toBuffer(),
    ]);
    const custodyTokenAccount = await ensureAta(provider, pair.token0, custodyAuthority, true);
    const executorTokenAccount = await ensureAta(provider, pair.token0, executor.publicKey);
    const ownerTokenAccount = userToken0;

    const beforeIx = await leverageDelegate.methods
        .beforeTakeProfit({ orderId: new BN(orderId) })
        .accounts({
            order,
            pair: SELECTED_PAIR,
            userLeveragePosition,
            userLeverageDelegation,
            custodyAuthority,
            custodyTokenAccount,
            tokenMint: pair.token0,
            executor: executor.publicKey,
        })
        .instruction();

    const afterIx = await leverageDelegate.methods
        .afterCloseOrder({ orderId: new BN(orderId) })
        .accounts({
            order,
            owner: owner.publicKey,
            userLeveragePosition,
            custodyAuthority,
            custodyTokenAccount,
            executorTokenAccount,
            ownerTokenAccount,
            tokenMint: pair.token0,
            executor: executor.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
        })
        .instruction();

    const beforeAccounts = [
        { pubkey: order, isWritable: true, isSigner: false },
        { pubkey: SELECTED_PAIR, isWritable: false, isSigner: false },
        { pubkey: userLeveragePosition, isWritable: false, isSigner: false },
        { pubkey: userLeverageDelegation, isWritable: false, isSigner: false },
        { pubkey: custodyAuthority, isWritable: false, isSigner: false },
        { pubkey: custodyTokenAccount, isWritable: false, isSigner: false },
        { pubkey: pair.token0, isWritable: false, isSigner: false },
        { pubkey: executor.publicKey, isWritable: false, isSigner: true },
    ];
    const afterAccounts = [
        { pubkey: order, isWritable: true, isSigner: false },
        { pubkey: owner.publicKey, isWritable: true, isSigner: false },
        { pubkey: userLeveragePosition, isWritable: false, isSigner: false },
        { pubkey: custodyAuthority, isWritable: false, isSigner: false },
        { pubkey: custodyTokenAccount, isWritable: true, isSigner: false },
        { pubkey: executorTokenAccount, isWritable: true, isSigner: false },
        { pubkey: ownerTokenAccount, isWritable: true, isSigner: false },
        { pubkey: pair.token0, isWritable: false, isSigner: false },
        { pubkey: executor.publicKey, isWritable: false, isSigner: true },
        { pubkey: TOKEN_PROGRAM_ID, isWritable: false, isSigner: false },
    ];

    const ownerBeforeClose = (await getAccount(connection, ownerTokenAccount)).amount;
    const executorBeforeClose = (await getAccount(connection, executorTokenAccount)).amount;
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
            pair: SELECTED_PAIR,
            rateModel: pair.rateModel,
            futarchyAuthority,
            positionOwner: owner.publicKey,
            userLeveragePosition,
            tokenInVault: reserve1Vault,
            tokenOutVault: reserve0Vault,
            leverageCollateralVault,
            recipientTokenOutAccount: custodyTokenAccount,
            tokenInMint: pair.token1,
            tokenOutMint: pair.token0,
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
    const custodyBalance = (await getAccount(connection, custodyTokenAccount)).amount;
    if (custodyBalance !== 0n) {
        throw new Error(`delegate custody should be empty after settlement, found ${custodyBalance}`);
    }
    const ownerAfterClose = (await getAccount(connection, ownerTokenAccount)).amount;
    const executorAfterClose = (await getAccount(connection, executorTokenAccount)).amount;
    if (ownerAfterClose <= ownerBeforeClose && executorAfterClose <= executorBeforeClose) {
        throw new Error('delegated close did not settle residual to owner or keeper');
    }

    console.log('\nSurfpool mainnet-fork leverage e2e passed.');
    console.log('Token0 decimals:', token0MintInfo.decimals);
    console.log('Token1 decimals:', token1MintInfo.decimals);
    console.log('Position closed:', userLeveragePosition.toBase58());
}

main().catch((error) => {
    console.error(error);
    process.exit(1);
});
