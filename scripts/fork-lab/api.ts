import { existsSync, readFileSync } from 'node:fs';
import http from 'node:http';
import { resolve } from 'node:path';
import * as anchor from '@coral-xyz/anchor';
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
    TransactionInstruction,
} from '@solana/web3.js';
import omnipairIdl from './idl/omnipair.json' with { type: 'json' };
import leverageDelegateIdl from './idl/leverage_delegate.json' with { type: 'json' };

const PORT = Number(process.env.PORT ?? process.env.FORK_API_PORT ?? 3011);
const SURFPOOL_RPC_URL = process.env.SURFPOOL_RPC_URL ?? 'http://127.0.0.1:8899';
const PUBLIC_RPC_URL = process.env.PUBLIC_SURFPOOL_RPC_URL ?? process.env.SURFPOOL_RPC_PROXY_URL ?? SURFPOOL_RPC_URL;
const ADMIN_TOKEN = process.env.FORK_ADMIN_TOKEN ?? '';
const ALLOW_PUBLIC_FUNDING = process.env.FORK_ALLOW_PUBLIC_FUNDING !== 'false';
const DEFAULT_PAIR = new PublicKey(
    process.env.SURFPOOL_LEVERAGE_PAIR ?? 'Cp2nGCWWfqkUmPR3pPKoR376Fti8wuYRFrSWJZq1a9SA',
);
const DEFAULT_FUNDING_RAW = BigInt(process.env.FORK_DEFAULT_TOKEN_FUNDING_RAW ?? '1000000000000');
const CLOSE_PERMISSION = 1 << 0;
const ORDER_KIND_TAKE_PROFIT = 1;
const BPS_DENOMINATOR = 10_000n;
const NAD = 1_000_000_000n;

type PairAccount = {
    token0: PublicKey;
    token1: PublicKey;
    rateModel: PublicKey;
    reserve0: BN;
    reserve1: BN;
    cashReserve0: BN;
    cashReserve1: BN;
    swapFeeBps: number;
    totalDebt0: BN;
    totalDebt1: BN;
    totalDebt0Shares: BN;
    totalDebt1Shares: BN;
    reduceOnly: boolean;
    fixedCfBps?: number | null;
};

function loadKeypair(): Keypair {
    const path = resolve(process.env.FORK_LAB_PAYER_KEYPAIR ?? process.env.ANCHOR_WALLET ?? 'deployer-keypair.json');
    if (!existsSync(path)) {
        throw new Error(`Fork API payer keypair not found at ${path}. Set FORK_LAB_PAYER_KEYPAIR or ANCHOR_WALLET.`);
    }
    return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(path, 'utf8'))));
}

const payer = loadKeypair();
const connection = new Connection(SURFPOOL_RPC_URL, 'confirmed');
const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(payer), { commitment: 'confirmed' });
anchor.setProvider(provider);

const omnipair: any = new anchor.Program(omnipairIdl as anchor.Idl, provider);
const leverageDelegate: any = new anchor.Program(leverageDelegateIdl as anchor.Idl, provider);

function seed(value: string): Buffer {
    return Buffer.from(value);
}

function u64(value: number | bigint): Buffer {
    const buffer = Buffer.alloc(8);
    buffer.writeBigUInt64LE(BigInt(value));
    return buffer;
}

function pda(programId: PublicKey, seeds: (Buffer | Uint8Array)[]): PublicKey {
    return PublicKey.findProgramAddressSync(seeds, programId)[0];
}

function toBN(value: bigint | number | string): BN {
    return new BN(value.toString());
}

function toBigInt(value: BN | number | bigint | string): bigint {
    if (typeof value === 'bigint') return value;
    if (typeof value === 'number') return BigInt(value);
    if (typeof value === 'string') return BigInt(value);
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
        throw new Error('Cannot compute a take-profit trigger for a zero-collateral position');
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

function reserveVault(pair: PublicKey, mint: PublicKey): PublicKey {
    return pda(omnipair.programId, [seed('reserve_vault'), pair.toBuffer(), mint.toBuffer()]);
}

function leveragePosition(pair: PublicKey, owner: PublicKey, isDebtToken0: boolean): PublicKey {
    return pda(omnipair.programId, [
        seed('leverage_position'),
        pair.toBuffer(),
        owner.toBuffer(),
        Buffer.from([isDebtToken0 ? 1 : 0]),
    ]);
}

function leverageDelegation(position: PublicKey): PublicKey {
    return pda(omnipair.programId, [seed('leverage_delegation'), position.toBuffer()]);
}

function leverageCollateralVault(pair: PublicKey, collateralMint: PublicKey): PublicKey {
    return pda(omnipair.programId, [seed('leverage_collateral_vault'), pair.toBuffer(), collateralMint.toBuffer()]);
}

function lpMint(pair: PublicKey): PublicKey {
    return pda(omnipair.programId, [seed('gamm_lp_mint'), pair.toBuffer()]);
}

function orderPda(position: PublicKey, owner: PublicKey, orderId: number | bigint): PublicKey {
    return pda(leverageDelegate.programId, [
        seed('leverage_order'),
        position.toBuffer(),
        owner.toBuffer(),
        u64(orderId),
    ]);
}

function custodyAuthority(order: PublicKey): PublicKey {
    return pda(leverageDelegate.programId, [seed('leverage_delegate_authority'), order.toBuffer()]);
}

function futarchyAuthority(): PublicKey {
    return pda(omnipair.programId, [seed('futarchy_authority')]);
}

function eventAuthority(): PublicKey {
    return pda(omnipair.programId, [seed('__event_authority')]);
}

function side(pair: PairAccount, pairKey: PublicKey, owner: PublicKey, isDebtToken0: boolean) {
    const debtMint = isDebtToken0 ? pair.token0 : pair.token1;
    const collateralMint = isDebtToken0 ? pair.token1 : pair.token0;
    const position = leveragePosition(pairKey, owner, isDebtToken0);
    return {
        debtMint,
        collateralMint,
        debtVault: reserveVault(pairKey, debtMint),
        collateralVault: reserveVault(pairKey, collateralMint),
        ownerDebtTokenAccount: getAssociatedTokenAddressSync(debtMint, owner),
        ownerCollateralTokenAccount: getAssociatedTokenAddressSync(collateralMint, owner),
        position,
        delegation: leverageDelegation(position),
        leverageCollateralVault: leverageCollateralVault(pairKey, collateralMint),
    };
}

async function rpcRequest(method: string, params: unknown[]) {
    const response = await fetch(SURFPOOL_RPC_URL, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
    });
    const payload = await response.json() as { result?: unknown; error?: unknown };
    if (payload.error) {
        throw new Error(`${method} failed: ${JSON.stringify(payload.error)}`);
    }
    return payload.result;
}

async function setLamports(pubkey: PublicKey, sol: number) {
    try {
        const signature = await connection.requestAirdrop(pubkey, sol * LAMPORTS_PER_SOL);
        await connection.confirmTransaction(signature, 'confirmed');
    } catch {
        await rpcRequest('surfnet_setAccount', [
            pubkey.toBase58(),
            {
                lamports: sol * LAMPORTS_PER_SOL,
                owner: SystemProgram.programId.toBase58(),
            },
        ]);
    }
}

async function setTokenBalance(owner: PublicKey, mint: PublicKey, amount: bigint) {
    if (amount > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new Error(`surfnet_setTokenAccount amount is above JSON safe integer range: ${amount}`);
    }
    await rpcRequest('surfnet_setTokenAccount', [
        owner.toBase58(),
        mint.toBase58(),
        {
            amount: Number(amount),
            state: 'initialized',
        },
        TOKEN_PROGRAM_ID.toBase58(),
    ]);
}

async function ensureAta(mint: PublicKey, owner: PublicKey, allowOwnerOffCurve = false): Promise<PublicKey> {
    const ata = getAssociatedTokenAddressSync(
        mint,
        owner,
        allowOwnerOffCurve,
        TOKEN_PROGRAM_ID,
        ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    const existing = await connection.getAccountInfo(ata, 'confirmed');
    if (!existing) {
        const tx = new Transaction().add(
            createAssociatedTokenAccountInstruction(
                payer.publicKey,
                ata,
                owner,
                mint,
                TOKEN_PROGRAM_ID,
                ASSOCIATED_TOKEN_PROGRAM_ID,
            ),
        );
        await provider.sendAndConfirm(tx, []);
    }
    return ata;
}

async function serializeOwnerTx(owner: PublicKey, instructions: TransactionInstruction[]) {
    const tx = new Transaction().add(...instructions);
    tx.feePayer = owner;
    tx.recentBlockhash = (await connection.getLatestBlockhash('confirmed')).blockhash;
    return tx.serialize({ requireAllSignatures: false, verifySignatures: false }).toString('base64');
}

function parsePubkey(value: unknown, label: string): PublicKey {
    if (typeof value !== 'string') {
        throw new Error(`${label} is required`);
    }
    return new PublicKey(value);
}

function parseBool(value: unknown, label: string): boolean {
    if (typeof value !== 'boolean') {
        throw new Error(`${label} must be a boolean`);
    }
    return value;
}

function parseRaw(value: unknown, label: string, fallback?: bigint): bigint {
    if (value === undefined || value === null || value === '') {
        if (fallback !== undefined) return fallback;
        throw new Error(`${label} is required`);
    }
    return BigInt(String(value));
}

function parsePair(value: unknown): PublicKey {
    return typeof value === 'string' && value ? new PublicKey(value) : DEFAULT_PAIR;
}

function isAdmin(req: http.IncomingMessage) {
    if (!ADMIN_TOKEN) return true;
    return req.headers.authorization === `Bearer ${ADMIN_TOKEN}` || req.headers['x-fork-admin-token'] === ADMIN_TOKEN;
}

function replacer(_key: string, value: unknown): unknown {
    if (typeof value === 'bigint') return value.toString();
    if (value instanceof PublicKey) return value.toBase58();
    if (BN.isBN(value)) return value.toString();
    return value;
}

function jsonSafe(value: unknown): unknown {
    return JSON.parse(JSON.stringify(value, replacer));
}

function corsHeaders() {
    return {
        'access-control-allow-origin': process.env.FORK_API_CORS_ORIGIN ?? '*',
        'access-control-allow-methods': 'GET, POST, OPTIONS',
        'access-control-allow-headers': 'content-type, authorization, x-fork-admin-token',
    };
}

function sendJson(res: http.ServerResponse, status: number, value: unknown) {
    res.writeHead(status, {
        'content-type': 'application/json',
        ...corsHeaders(),
    });
    res.end(JSON.stringify(value, replacer));
}

async function readBody(req: http.IncomingMessage): Promise<any> {
    const chunks: Buffer[] = [];
    for await (const chunk of req) {
        chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
    }
    const text = Buffer.concat(chunks).toString('utf8');
    return text ? JSON.parse(text) : {};
}

async function pairPayload(pairKey: PublicKey) {
    const pair = await omnipair.account.pair.fetch(pairKey) as PairAccount;
    return {
        pairAddress: pairKey,
        programId: omnipair.programId,
        leverageDelegateProgramId: leverageDelegate.programId,
        token0: pair.token0,
        token1: pair.token1,
        rateModel: pair.rateModel,
        reserve0: pair.reserve0,
        reserve1: pair.reserve1,
        cashReserve0: pair.cashReserve0,
        cashReserve1: pair.cashReserve1,
        totalDebt0: pair.totalDebt0,
        totalDebt1: pair.totalDebt1,
        totalDebt0Shares: pair.totalDebt0Shares,
        totalDebt1Shares: pair.totalDebt1Shares,
        swapFeeBps: pair.swapFeeBps,
        reduceOnly: pair.reduceOnly,
        reserve0Vault: reserveVault(pairKey, pair.token0),
        reserve1Vault: reserveVault(pairKey, pair.token1),
    };
}

function tokenLabel(mint: PublicKey) {
    const address = mint.toBase58();
    if (address === 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v') {
        return { symbol: 'USDC', name: 'USD Coin' };
    }
    if (address === 'METAwkXcqyXKy1AtsSgJ8JiUHwGCafnZL38n3vYmeta') {
        return { symbol: 'META', name: 'MetaDAO' };
    }
    return { symbol: address.slice(0, 4), name: address };
}

function rawToUi(amount: bigint, decimals: number): string {
    const scale = 10n ** BigInt(decimals);
    const whole = amount / scale;
    const fraction = amount % scale;
    if (fraction === 0n) return whole.toString();
    const padded = fraction.toString().padStart(decimals, '0').replace(/0+$/, '');
    return `${whole}.${padded}`;
}

async function apiPoolPayload(pairKey: PublicKey) {
    const pair = await omnipair.account.pair.fetch(pairKey) as PairAccount;
    const [mint0, mint1] = await Promise.all([
        getMint(connection, pair.token0),
        getMint(connection, pair.token1),
    ]);
    const token0 = tokenLabel(pair.token0);
    const token1 = tokenLabel(pair.token1);
    const reserve0Ui = rawToUi(toBigInt(pair.reserve0), mint0.decimals);
    const reserve1Ui = rawToUi(toBigInt(pair.reserve1), mint1.decimals);
    const debt0Ui = rawToUi(toBigInt(pair.totalDebt0), mint0.decimals);
    const debt1Ui = rawToUi(toBigInt(pair.totalDebt1), mint1.decimals);
    const reserve0Num = Number(reserve0Ui);
    const reserve1Num = Number(reserve1Ui);
    const token0Price = reserve0Num > 0 ? reserve1Num / reserve0Num : 0;
    const token1Price = reserve1Num > 0 ? reserve0Num / reserve1Num : 0;

    return {
        id: 0,
        pair_address: pairKey.toBase58(),
        token0: {
            ...token0,
            decimals: mint0.decimals,
            address: pair.token0.toBase58(),
            icon: null,
        },
        token1: {
            ...token1,
            decimals: mint1.decimals,
            address: pair.token1.toBase58(),
            icon: null,
        },
        reserves: {
            token0: reserve0Ui,
            token1: reserve1Ui,
        },
        oracle_prices: {
            token0: token0Price.toString(),
            token1: token1Price.toString(),
        },
        spot_prices: {
            token0: token0Price.toString(),
            token1: token1Price.toString(),
        },
        interest_rates: {
            token0: 0,
            token1: 0,
        },
        total_debts: {
            token0: debt0Ui,
            token1: debt1Ui,
        },
        utilization: {
            token0: toBigInt(pair.reserve0) > 0n ? Number(toBigInt(pair.totalDebt0)) / Number(toBigInt(pair.reserve0)) : 0,
            token1: toBigInt(pair.reserve1) > 0n ? Number(toBigInt(pair.totalDebt1)) / Number(toBigInt(pair.reserve1)) : 0,
        },
        lp_token: {
            address: lpMint(pairKey).toBase58(),
            total_supply: '0',
            decimals: 6,
        },
        swap_fee_bps: String(pair.swapFeeBps),
        fixed_cf_bps: pair.fixedCfBps === null || pair.fixedCfBps === undefined ? null : String(pair.fixedCfBps),
    };
}

async function leveragePositionsPayload(owner: PublicKey, pairKey: PublicKey) {
    const pair = await omnipair.account.pair.fetch(pairKey) as PairAccount;
    const results = [];
    for (const isDebtToken0 of [false, true]) {
        const derived = side(pair, pairKey, owner, isDebtToken0);
        const info = await connection.getAccountInfo(derived.position, 'confirmed');
        if (!info) continue;
        const position = await omnipair.account.userLeveragePosition.fetch(derived.position);
        const delegationInfo = await connection.getAccountInfo(derived.delegation, 'confirmed');
        const delegation = delegationInfo
            ? await omnipair.account.userLeverageDelegation.fetch(derived.delegation)
            : null;
        results.push({
            address: derived.position,
            isDebtToken0,
            debtMint: derived.debtMint,
            collateralMint: derived.collateralMint,
            leverageCollateralVault: derived.leverageCollateralVault,
            delegationAddress: derived.delegation,
            delegation,
            position,
        });
    }
    return results;
}

async function leverageOrdersPayload(owner: PublicKey, pairKey: PublicKey, positionFilter?: PublicKey) {
    const orders = await leverageDelegate.account.leverageOrder.all();
    return orders
        .filter(({ account }: any) => {
            return (
                account.owner.equals(owner) &&
                account.pair.equals(pairKey) &&
                (!positionFilter || account.position.equals(positionFilter))
            );
        })
        .map(({ publicKey, account }: any) => ({
            address: publicKey,
            order: account,
        }));
}

async function buildDelegatedCloseIxs(
    pairKey: PublicKey,
    orderId: number,
    order: PublicKey,
    owner: PublicKey,
    executor: PublicKey,
    position: PublicKey,
    delegation: PublicKey,
    custodyAuth: PublicKey,
    custodyTokenAccount: PublicKey,
    executorTokenAccount: PublicKey,
    ownerTokenAccount: PublicKey,
    tokenMint: PublicKey,
) {
    const [beforeIx, afterIx] = await Promise.all([
        leverageDelegate.methods
            .beforeTakeProfit({ orderId: new BN(orderId) })
            .accounts({
                order,
                pair: pairKey,
                userLeveragePosition: position,
                userLeverageDelegation: delegation,
                custodyAuthority: custodyAuth,
                custodyTokenAccount,
                tokenMint,
                executor,
            })
            .instruction(),
        leverageDelegate.methods
            .afterCloseOrder({ orderId: new BN(orderId) })
            .accounts({
                order,
                owner,
                userLeveragePosition: position,
                custodyAuthority: custodyAuth,
                custodyTokenAccount,
                executorTokenAccount,
                ownerTokenAccount,
                tokenMint,
                executor,
                tokenProgram: TOKEN_PROGRAM_ID,
            })
            .instruction(),
    ]);

    const beforeAccounts = [
        { pubkey: order, isWritable: true, isSigner: false },
        { pubkey: pairKey, isWritable: false, isSigner: false },
        { pubkey: position, isWritable: false, isSigner: false },
        { pubkey: delegation, isWritable: false, isSigner: false },
        { pubkey: custodyAuth, isWritable: false, isSigner: false },
        { pubkey: custodyTokenAccount, isWritable: false, isSigner: false },
        { pubkey: tokenMint, isWritable: false, isSigner: false },
        { pubkey: executor, isWritable: false, isSigner: true },
    ];
    const afterAccounts = [
        { pubkey: order, isWritable: true, isSigner: false },
        { pubkey: owner, isWritable: true, isSigner: false },
        { pubkey: position, isWritable: false, isSigner: false },
        { pubkey: custodyAuth, isWritable: false, isSigner: false },
        { pubkey: custodyTokenAccount, isWritable: true, isSigner: false },
        { pubkey: executorTokenAccount, isWritable: true, isSigner: false },
        { pubkey: ownerTokenAccount, isWritable: true, isSigner: false },
        { pubkey: tokenMint, isWritable: false, isSigner: false },
        { pubkey: executor, isWritable: false, isSigner: true },
        { pubkey: TOKEN_PROGRAM_ID, isWritable: false, isSigner: false },
    ];

    return { beforeIx, afterIx, beforeAccounts, afterAccounts };
}

async function route(req: http.IncomingMessage, body: any) {
    const url = new URL(req.url ?? '/', `http://${req.headers.host ?? 'localhost'}`);
    const pathname = url.pathname;

    if (req.method === 'GET' && pathname === '/health') {
        return { ok: true, rpcUrl: SURFPOOL_RPC_URL, publicRpcUrl: PUBLIC_RPC_URL };
    }

    if (req.method === 'GET' && pathname === '/api/v1/fork/config') {
        return {
            rpcUrl: PUBLIC_RPC_URL,
            pair: DEFAULT_PAIR,
            omnipairProgramId: omnipair.programId,
            leverageDelegateProgramId: leverageDelegate.programId,
        };
    }

    if (req.method === 'GET' && pathname === '/api/v1/fork/pair') {
        return jsonSafe(await pairPayload(parsePair(url.searchParams.get('pair'))));
    }

    if (req.method === 'GET' && pathname === '/api/v1/pools') {
        const pool = await apiPoolPayload(parsePair(url.searchParams.get('pair')));
        return { success: true, data: { pools: [pool], count: 1 } };
    }

    if (req.method === 'GET' && pathname.startsWith('/api/v1/pools/')) {
        const pairFromPath = pathname.split('/').pop();
        const pool = await apiPoolPayload(parsePair(pairFromPath));
        return { success: true, data: pool };
    }

    if (req.method === 'GET' && pathname === '/api/v1/fork/leverage/positions') {
        const owner = parsePubkey(url.searchParams.get('owner'), 'owner');
        const pairKey = parsePair(url.searchParams.get('pair'));
        return { data: jsonSafe(await leveragePositionsPayload(owner, pairKey)) };
    }

    if (req.method === 'GET' && pathname === '/api/v1/fork/leverage/orders') {
        const owner = parsePubkey(url.searchParams.get('owner'), 'owner');
        const pairKey = parsePair(url.searchParams.get('pair'));
        const position = url.searchParams.get('position') ? new PublicKey(url.searchParams.get('position')!) : undefined;
        return { data: jsonSafe(await leverageOrdersPayload(owner, pairKey, position)) };
    }

    if (req.method === 'POST' && pathname === '/api/v1/fork/fund-wallet') {
        if (!ALLOW_PUBLIC_FUNDING && !isAdmin(req)) {
            throw new Error('fork funding requires FORK_ADMIN_TOKEN');
        }
        const owner = parsePubkey(body.owner ?? body.wallet, 'owner');
        const pairKey = parsePair(body.pair);
        const pair = await omnipair.account.pair.fetch(pairKey) as PairAccount;
        const sol = Number(body.sol ?? 25);
        await setLamports(owner, sol);
        await Promise.all([
            setTokenBalance(owner, pair.token0, parseRaw(body.token0Amount, 'token0Amount', DEFAULT_FUNDING_RAW)),
            setTokenBalance(owner, pair.token1, parseRaw(body.token1Amount, 'token1Amount', DEFAULT_FUNDING_RAW)),
        ]);
        return {
            ok: true,
            owner,
            token0: pair.token0,
            token1: pair.token1,
            token0Account: getAssociatedTokenAddressSync(pair.token0, owner),
            token1Account: getAssociatedTokenAddressSync(pair.token1, owner),
        };
    }

    if (req.method === 'POST' && pathname === '/api/v1/fork/tx/open-leverage') {
        const owner = parsePubkey(body.owner, 'owner');
        const pairKey = parsePair(body.pair);
        const isDebtToken0 = parseBool(body.isDebtToken0, 'isDebtToken0');
        const marginAmount = parseRaw(body.marginAmount, 'marginAmount');
        const multiplierBps = Number(body.multiplierBps ?? 20_000);
        const minCollateralOut = parseRaw(body.minCollateralOut, 'minCollateralOut', 0n);
        const pair = await omnipair.account.pair.fetch(pairKey) as PairAccount;
        const derived = side(pair, pairKey, owner, isDebtToken0);

        const ix = await omnipair.methods
            .openLeverage({
                isDebtToken0,
                marginAmount: toBN(marginAmount),
                multiplierBps: new BN(multiplierBps),
                minCollateralOut: toBN(minCollateralOut),
            })
            .accounts({
                pair: pairKey,
                rateModel: pair.rateModel,
                futarchyAuthority: futarchyAuthority(),
                userLeveragePosition: derived.position,
                tokenInVault: derived.debtVault,
                tokenOutVault: derived.collateralVault,
                leverageCollateralVault: derived.leverageCollateralVault,
                userTokenInAccount: derived.ownerDebtTokenAccount,
                tokenInMint: derived.debtMint,
                tokenOutMint: derived.collateralMint,
                user: owner,
                tokenProgram: TOKEN_PROGRAM_ID,
                token2022Program: TOKEN_2022_PROGRAM_ID,
                systemProgram: SystemProgram.programId,
                eventAuthority: eventAuthority(),
                program: omnipair.programId,
            })
            .instruction();

        return {
            transaction: await serializeOwnerTx(owner, [ix]),
            pair: pairKey,
            owner,
            userLeveragePosition: derived.position,
            isDebtToken0,
            debtMint: derived.debtMint,
            collateralMint: derived.collateralMint,
        };
    }

    if (req.method === 'POST' && pathname === '/api/v1/fork/tx/create-current-price-take-profit') {
        const owner = parsePubkey(body.owner, 'owner');
        const pairKey = parsePair(body.pair);
        const isDebtToken0 = parseBool(body.isDebtToken0, 'isDebtToken0');
        const orderId = Number(body.orderId ?? Date.now());
        const pair = await omnipair.account.pair.fetch(pairKey) as PairAccount;
        const derived = side(pair, pairKey, owner, isDebtToken0);
        const position = await omnipair.account.userLeveragePosition.fetch(derived.position);
        const current = closeoutPricePerUnitNad(pair, position);
        const trigger = parseRaw(body.triggerCloseoutPriceNad, 'triggerCloseoutPriceNad', current.closeoutPriceNad);
        const order = orderPda(derived.position, owner, orderId);

        const ixs: TransactionInstruction[] = [];
        if (!(await connection.getAccountInfo(derived.delegation, 'confirmed'))) {
            ixs.push(await omnipair.methods
                .createLeverageDelegation({
                    isDebtToken0,
                    delegatedProgram: leverageDelegate.programId,
                    approvedActions: CLOSE_PERMISSION,
                })
                .accounts({
                    pair: pairKey,
                    userLeveragePosition: derived.position,
                    userLeverageDelegation: derived.delegation,
                    owner,
                    systemProgram: SystemProgram.programId,
                })
                .instruction());
        }
        ixs.push(await leverageDelegate.methods
            .createLeverageOrder({
                orderId: new BN(orderId),
                kind: ORDER_KIND_TAKE_PROFIT,
                triggerCloseoutPriceNad: toBN(trigger),
            })
            .accounts({
                pair: pairKey,
                userLeveragePosition: derived.position,
                order,
                owner,
                systemProgram: SystemProgram.programId,
            })
            .instruction());

        return {
            transaction: await serializeOwnerTx(owner, ixs),
            pair: pairKey,
            owner,
            order,
            orderId,
            userLeveragePosition: derived.position,
            userLeverageDelegation: derived.delegation,
            triggerCloseoutPriceNad: trigger,
            currentCloseoutValue: current.closeoutValue,
        };
    }

    if (req.method === 'POST' && pathname === '/api/v1/fork/keeper/execute-take-profit') {
        const owner = parsePubkey(body.owner, 'owner');
        const pairKey = parsePair(body.pair);
        const isDebtToken0 = parseBool(body.isDebtToken0, 'isDebtToken0');
        const orderId = Number(body.orderId);
        if (!Number.isFinite(orderId)) throw new Error('orderId is required');

        const pair = await omnipair.account.pair.fetch(pairKey) as PairAccount;
        const derived = side(pair, pairKey, owner, isDebtToken0);
        const order = orderPda(derived.position, owner, orderId);
        const custodyAuth = custodyAuthority(order);
        const executor = payer.publicKey;
        const custodyTokenAccount = await ensureAta(derived.debtMint, custodyAuth, true);
        const executorTokenAccount = await ensureAta(derived.debtMint, executor);
        const ownerTokenAccount = await ensureAta(derived.debtMint, owner);
        const closeIxs = await buildDelegatedCloseIxs(
            pairKey,
            orderId,
            order,
            owner,
            executor,
            derived.position,
            derived.delegation,
            custodyAuth,
            custodyTokenAccount,
            executorTokenAccount,
            ownerTokenAccount,
            derived.debtMint,
        );

        const ownerBefore = (await getAccount(connection, ownerTokenAccount)).amount;
        const executorBefore = (await getAccount(connection, executorTokenAccount)).amount;
        const signature = await omnipair.methods
            .delegatedCloseLeverage({
                isDebtToken0,
                minAmountOut: new BN(body.minAmountOut ?? 0),
                delegated: {
                    beforeIxData: closeIxs.beforeIx.data,
                    afterIxData: closeIxs.afterIx.data,
                    beforeAccountsLen: closeIxs.beforeAccounts.length,
                },
            })
            .accounts({
                pair: pairKey,
                rateModel: pair.rateModel,
                futarchyAuthority: futarchyAuthority(),
                positionOwner: owner,
                userLeveragePosition: derived.position,
                tokenInVault: derived.collateralVault,
                tokenOutVault: derived.debtVault,
                leverageCollateralVault: derived.leverageCollateralVault,
                recipientTokenOutAccount: custodyTokenAccount,
                tokenInMint: derived.collateralMint,
                tokenOutMint: derived.debtMint,
                userLeverageDelegation: derived.delegation,
                delegatedProgram: leverageDelegate.programId,
                authority: executor,
                tokenProgram: TOKEN_PROGRAM_ID,
                token2022Program: TOKEN_2022_PROGRAM_ID,
                systemProgram: SystemProgram.programId,
                eventAuthority: eventAuthority(),
                program: omnipair.programId,
            })
            .remainingAccounts([...closeIxs.beforeAccounts, ...closeIxs.afterAccounts])
            .rpc();

        const custodyBalance = (await getAccount(connection, custodyTokenAccount)).amount;
        const ownerAfter = (await getAccount(connection, ownerTokenAccount)).amount;
        const executorAfter = (await getAccount(connection, executorTokenAccount)).amount;
        return {
            signature,
            order,
            userLeveragePosition: derived.position,
            custodyTokenAccount,
            ownerTokenAccount,
            executorTokenAccount,
            custodyBalance,
            ownerDelta: ownerAfter - ownerBefore,
            executorDelta: executorAfter - executorBefore,
        };
    }

    throw new Error(`Unknown route ${req.method} ${pathname}`);
}

const server = http.createServer(async (req, res) => {
    if (req.method === 'OPTIONS') {
        res.writeHead(204, corsHeaders());
        res.end();
        return;
    }

    try {
        const body = req.method === 'POST' ? await readBody(req) : {};
        const value = await route(req, body);
        sendJson(res, 200, value);
    } catch (error) {
        sendJson(res, 400, {
            error: error instanceof Error ? error.message : String(error),
        });
    }
});

server.listen(PORT, '0.0.0.0', () => {
    console.log(`fork API listening on :${PORT}`);
    console.log(`Surfpool RPC: ${SURFPOOL_RPC_URL}`);
    console.log(`Public RPC: ${PUBLIC_RPC_URL}`);
    console.log(`Omnipair: ${omnipair.programId.toBase58()}`);
    console.log(`Leverage delegate: ${leverageDelegate.programId.toBase58()}`);
});
