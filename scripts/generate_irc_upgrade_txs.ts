import {
    Connection,
    Keypair,
    PublicKey,
    SystemProgram,
    TransactionInstruction,
    TransactionMessage,
    VersionedTransaction,
} from '@solana/web3.js';
import { AnchorProvider, Program } from '@coral-xyz/anchor';
import BN from 'bn.js';
import * as dotenv from 'dotenv';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import idl from '../target/idl/omnipair.json' with { type: 'json' };
import type { Omnipair } from '../target/types/omnipair';

const MS_PER_DAY = 86_400_000;
const NAD = new BN(1_000_000_000);
const BPS_DENOMINATOR = 10_000;
const DEFAULT_BATCH_BYTE_LIMIT = 1232;
const DEFAULT_CONCURRENCY = 8;

const TARGET_RATE_MODEL = {
    targetUtilStartBps: 2_500,
    targetUtilEndBps: 4_500,
    halfLifeMs: 7 * MS_PER_DAY,
    minRateBps: 300,
};

type Source = 'db' | 'chain' | 'pairs' | 'file';

type CliOptions = {
    source?: Source;
    pairs: string[];
    pairsFile?: string;
    rpcUrl?: string;
    databaseUrl?: string;
    indexerEnvPath?: string;
    authority?: string;
    feePayer?: string;
    outDir?: string;
    batchByteLimit: number;
    maxInstructionsPerTx?: number;
    visibleOnly: boolean;
    includeAlreadyTarget: boolean;
    shareRateModels: boolean;
    signRateModels: boolean;
    writeKeypairs: boolean;
    dryRun: boolean;
    printBase64: boolean;
    concurrency: number;
};

type DbPairRow = {
    pair_address: string;
    token0?: string;
    token1?: string;
    rate_model?: string;
    visible?: boolean;
};

type PairCandidate = {
    pair: PublicKey;
    db?: DbPairRow;
};

type RateModelParams = {
    targetUtilStartBps: number;
    targetUtilEndBps: number;
    halfLifeMs: number;
    minRateBps: number;
    maxRateBps: number;
    initialRateBps: number;
};

type PairPlan = {
    pair: PublicKey;
    currentRateModel: PublicKey;
    currentParams: RateModelParams;
    newParams: RateModelParams;
    db?: DbPairRow;
};

type RateModelPlan = {
    keypair: Keypair;
    params: RateModelParams;
    pairs: PairPlan[];
};

type InstructionItem = {
    kind: 'create-rate-model' | 'set-pair-rate-model';
    ix: TransactionInstruction;
    rateModel: RateModelPlan;
    pair?: PairPlan;
};

type BuiltTransaction = {
    index: number;
    base64: string;
    byteLength: number;
    instructionCount: number;
    createRateModels: string[];
    setPairs: string[];
};

function usage(): string {
    return `
Usage:
  yarn generate-irc-upgrade [options]

Builds unsigned base64 v0 transactions that:
  1. create cloned RateModel accounts with:
     - half_life_ms = 7 days
     - min_rate_bps = 300
     - target_util_start/end = 2500/4500
  2. point selected pairs at the new RateModel account(s).

Sources:
  --source db              Read pair addresses from omnipair-indexer Postgres (default)
  --source chain           Fetch all Pair accounts from the program
  --pairs A,B,C            Use an explicit comma/space separated pair list
  --pairs-file ./pairs.txt Use a file containing pair addresses

Common options:
  --indexer-env PATH       Env file containing DATABASE_URL/SOLANA_RPC_URL
                           Defaults to ../omnipair-indexer/api/.env when present
  --rpc URL                Override RPC URL
  --db-url URL             Override DATABASE_URL
  --authority PUBKEY       Override futarchy authority signer
  --fee-payer PUBKEY       Override transaction fee payer (defaults to authority)
  --include-hidden         Include DB pools where visible=false
  --include-already-target Do not skip pairs that already match the target params
  --one-rate-model-per-pair Create a fresh RateModel per pair instead of sharing identical params
  --batch-byte-limit N     Default ${DEFAULT_BATCH_BYTE_LIMIT}
  --max-instructions-per-tx N
  --out-dir DIR            Default .generated/irc-upgrade-<timestamp>
  --dry-run                Build and summarize without writing files
  --print-base64           Also print base64 payloads to stdout
  --sign-rate-models       Partially sign create txs with generated RateModel keypairs
  --no-write-keypairs      Do not write generated RateModel keypair files
`;
}

function parseArgs(argv: string[]): CliOptions {
    const opts: CliOptions = {
        pairs: [],
        batchByteLimit: DEFAULT_BATCH_BYTE_LIMIT,
        visibleOnly: true,
        includeAlreadyTarget: false,
        shareRateModels: true,
        signRateModels: false,
        writeKeypairs: true,
        dryRun: false,
        printBase64: false,
        concurrency: DEFAULT_CONCURRENCY,
    };

    const readValue = (arg: string, index: number): [string, number] => {
        const eq = arg.indexOf('=');
        if (eq >= 0) {
            return [arg.slice(eq + 1), index];
        }
        const value = argv[index + 1];
        if (!value || value.startsWith('--')) {
            throw new Error(`Missing value for ${arg}`);
        }
        return [value, index + 1];
    };

    for (let i = 0; i < argv.length; i += 1) {
        const arg = argv[i];
        if (arg === '--help' || arg === '-h') {
            console.log(usage());
            process.exit(0);
        }

        if (arg === '--include-hidden' || arg === '--all-pools') {
            opts.visibleOnly = false;
            continue;
        }
        if (arg === '--include-already-target') {
            opts.includeAlreadyTarget = true;
            continue;
        }
        if (arg === '--one-rate-model-per-pair') {
            opts.shareRateModels = false;
            continue;
        }
        if (arg === '--sign-rate-models') {
            opts.signRateModels = true;
            continue;
        }
        if (arg === '--no-write-keypairs') {
            opts.writeKeypairs = false;
            continue;
        }
        if (arg === '--dry-run') {
            opts.dryRun = true;
            continue;
        }
        if (arg === '--print-base64') {
            opts.printBase64 = true;
            continue;
        }

        const [value, nextIndex] = readValue(arg, i);
        i = nextIndex;

        switch (arg.split('=')[0]) {
            case '--source':
                if (!['db', 'chain', 'pairs', 'file'].includes(value)) {
                    throw new Error(`Invalid --source: ${value}`);
                }
                opts.source = value as Source;
                break;
            case '--pairs':
                opts.pairs.push(...splitAddresses(value));
                break;
            case '--pairs-file':
                opts.pairsFile = value;
                break;
            case '--rpc':
                opts.rpcUrl = value;
                break;
            case '--db-url':
                opts.databaseUrl = value;
                break;
            case '--indexer-env':
                opts.indexerEnvPath = value;
                break;
            case '--authority':
                opts.authority = value;
                break;
            case '--fee-payer':
                opts.feePayer = value;
                break;
            case '--out-dir':
                opts.outDir = value;
                break;
            case '--batch-byte-limit':
                opts.batchByteLimit = parsePositiveInteger(value, '--batch-byte-limit');
                break;
            case '--max-instructions-per-tx':
                opts.maxInstructionsPerTx = parsePositiveInteger(value, '--max-instructions-per-tx');
                break;
            case '--concurrency':
                opts.concurrency = parsePositiveInteger(value, '--concurrency');
                break;
            default:
                throw new Error(`Unknown argument: ${arg}`);
        }
    }

    if (opts.pairs.length > 0 && !opts.source) {
        opts.source = 'pairs';
    }
    if (opts.pairsFile && !opts.source) {
        opts.source = 'file';
    }
    opts.source ??= 'db';

    return opts;
}

function parsePositiveInteger(raw: string, name: string): number {
    const value = Number(raw);
    if (!Number.isInteger(value) || value <= 0) {
        throw new Error(`${name} must be a positive integer`);
    }
    return value;
}

function splitAddresses(raw: string): string[] {
    return raw
        .split(/[\s,]+/g)
        .map((item) => item.trim())
        .filter(Boolean);
}

function loadEnv(opts: CliOptions): void {
    const repoRoot = getRepoRoot();
    dotenv.config({ path: path.join(repoRoot, '.env'), override: false });

    const defaultIndexerEnv = path.resolve(repoRoot, '../omnipair-indexer/api/.env');
    const indexerEnv = opts.indexerEnvPath
        ? path.resolve(opts.indexerEnvPath)
        : defaultIndexerEnv;

    if (fs.existsSync(indexerEnv)) {
        dotenv.config({ path: indexerEnv, override: false });
    }
}

function getRepoRoot(): string {
    const scriptPath = fileURLToPath(import.meta.url);
    return path.resolve(path.dirname(scriptPath), '..');
}

function getRpcUrl(opts: CliOptions): string {
    const rpcUrl =
        opts.rpcUrl ||
        process.env.ANCHOR_PROVIDER_URL ||
        process.env.SOLANA_RPC_URL ||
        process.env.MAINNET_NETWORK_URL ||
        process.env.DEVNET_NETWORK_URL;

    if (!rpcUrl) {
        throw new Error('Missing RPC URL. Set --rpc, ANCHOR_PROVIDER_URL, or SOLANA_RPC_URL.');
    }
    return rpcUrl;
}

function getProgramId(opts: CliOptions): PublicKey {
    const programId = process.env.OMNIPAIR_PROGRAM_ID || (idl as { address?: string }).address;
    if (!programId) {
        throw new Error('Missing program id. Set OMNIPAIR_PROGRAM_ID or use an IDL with address.');
    }
    return new PublicKey(programId);
}

function makeProvider(connection: Connection, walletPubkey: PublicKey): AnchorProvider {
    const wallet = {
        publicKey: walletPubkey,
        signTransaction: async () => {
            throw new Error('This script only builds transactions; it does not wallet-sign them.');
        },
        signAllTransactions: async () => {
            throw new Error('This script only builds transactions; it does not wallet-sign them.');
        },
    };

    return new AnchorProvider(connection, wallet as never, {
        commitment: 'confirmed',
        preflightCommitment: 'confirmed',
    });
}

function queryPairsFromDb(databaseUrl: string, visibleOnly: boolean): DbPairRow[] {
    const whereVisible = visibleOnly ? 'WHERE COALESCE(visible, true) = true' : '';
    const sql = `
        SELECT COALESCE(json_agg(row_to_json(p)), '[]'::json)::text
        FROM (
            SELECT pair_address, token0, token1, rate_model, visible
            FROM pools
            ${whereVisible}
            ORDER BY id ASC
        ) p
    `;

    const stdout = execFileSync(
        'psql',
        [
            databaseUrl,
            '-v',
            'ON_ERROR_STOP=1',
            '-At',
            '-c',
            sql,
        ],
        {
            encoding: 'utf8',
            stdio: ['ignore', 'pipe', 'pipe'],
            env: { ...process.env, PAGER: 'cat' },
        },
    );

    return JSON.parse(stdout.trim() || '[]') as DbPairRow[];
}

async function loadCandidates(
    opts: CliOptions,
    program: Program<Omnipair>,
): Promise<PairCandidate[]> {
    if (opts.source === 'pairs') {
        return opts.pairs.map((pair) => ({ pair: new PublicKey(pair) }));
    }

    if (opts.source === 'file') {
        if (!opts.pairsFile) {
            throw new Error('--source file requires --pairs-file');
        }
        const raw = fs.readFileSync(path.resolve(opts.pairsFile), 'utf8');
        return splitAddresses(raw).map((pair) => ({ pair: new PublicKey(pair) }));
    }

    if (opts.source === 'chain') {
        const accounts = await program.account.pair.all();
        return accounts
            .map((account) => ({ pair: account.publicKey }))
            .sort((a, b) => a.pair.toBase58().localeCompare(b.pair.toBase58()));
    }

    const databaseUrl = opts.databaseUrl || process.env.DATABASE_URL;
    if (!databaseUrl) {
        throw new Error(
            'Missing DATABASE_URL for --source db. Set --db-url, DATABASE_URL, or use --source chain/--pairs.',
        );
    }

    const rows = queryPairsFromDb(databaseUrl, opts.visibleOnly);
    return rows.map((row) => ({
        pair: new PublicKey(row.pair_address),
        db: row,
    }));
}

async function buildPairPlans(
    candidates: PairCandidate[],
    program: Program<Omnipair>,
    includeAlreadyTarget: boolean,
    concurrency: number,
): Promise<PairPlan[]> {
    const plans: Array<PairPlan | null> = await mapWithConcurrency(candidates, concurrency, async (candidate) => {
        const pairAccount = await program.account.pair.fetch(candidate.pair);
        const currentRateModel = new PublicKey(pairAccount.rateModel);
        const currentRateModelAccount = await program.account.rateModel.fetch(currentRateModel);
        const currentParams = rateModelAccountToParams(currentRateModelAccount);
        const newParams = cloneWithTargetInterestParams(currentParams, candidate.pair);

        if (!includeAlreadyTarget && paramsEqual(currentParams, newParams)) {
            return null;
        }

        const plan: PairPlan = {
            pair: candidate.pair,
            currentRateModel,
            currentParams,
            newParams,
            db: candidate.db,
        };
        return plan;
    });

    return plans.filter((plan): plan is PairPlan => plan !== null);
}

function rateModelAccountToParams(rateModel: {
    targetUtilStart: BN;
    targetUtilEnd: BN;
    halfLifeMs: BN;
    minRate: BN;
    maxRate: BN;
    initialRate: BN;
}): RateModelParams {
    return {
        targetUtilStartBps: nadToBps(rateModel.targetUtilStart),
        targetUtilEndBps: nadToBps(rateModel.targetUtilEnd),
        halfLifeMs: bnToSafeNumber(rateModel.halfLifeMs, 'halfLifeMs'),
        minRateBps: nadToBps(rateModel.minRate),
        maxRateBps: nadToBps(rateModel.maxRate),
        initialRateBps: nadToBps(rateModel.initialRate),
    };
}

function nadToBps(value: BN): number {
    return value.muln(BPS_DENOMINATOR).div(NAD).toNumber();
}

function bnToSafeNumber(value: BN, name: string): number {
    const asNumber = value.toNumber();
    if (!Number.isSafeInteger(asNumber)) {
        throw new Error(`${name} is too large for JS number: ${value.toString()}`);
    }
    return asNumber;
}

function cloneWithTargetInterestParams(
    current: RateModelParams,
    pair: PublicKey,
): RateModelParams {
    if (current.maxRateBps > 0 && current.maxRateBps < TARGET_RATE_MODEL.minRateBps) {
        throw new Error(
            `Pair ${pair.toBase58()} has max_rate_bps=${current.maxRateBps}, below target min_rate_bps=${TARGET_RATE_MODEL.minRateBps}`,
        );
    }

    return {
        targetUtilStartBps: TARGET_RATE_MODEL.targetUtilStartBps,
        targetUtilEndBps: TARGET_RATE_MODEL.targetUtilEndBps,
        halfLifeMs: TARGET_RATE_MODEL.halfLifeMs,
        minRateBps: TARGET_RATE_MODEL.minRateBps,
        maxRateBps: current.maxRateBps,
        initialRateBps: Math.max(current.initialRateBps, TARGET_RATE_MODEL.minRateBps),
    };
}

function paramsEqual(a: RateModelParams, b: RateModelParams): boolean {
    return paramsKey(a) === paramsKey(b);
}

function paramsKey(params: RateModelParams): string {
    return [
        params.targetUtilStartBps,
        params.targetUtilEndBps,
        params.halfLifeMs,
        params.minRateBps,
        params.maxRateBps,
        params.initialRateBps,
    ].join(':');
}

function groupRateModels(pairPlans: PairPlan[], shareRateModels: boolean): RateModelPlan[] {
    if (!shareRateModels) {
        return pairPlans.map((pair) => ({
            keypair: Keypair.generate(),
            params: pair.newParams,
            pairs: [pair],
        }));
    }

    const byParams = new Map<string, RateModelPlan>();
    for (const pair of pairPlans) {
        const key = paramsKey(pair.newParams);
        let plan = byParams.get(key);
        if (!plan) {
            plan = {
                keypair: Keypair.generate(),
                params: pair.newParams,
                pairs: [],
            };
            byParams.set(key, plan);
        }
        plan.pairs.push(pair);
    }

    return [...byParams.values()];
}

async function createInstructionItems(
    program: Program<Omnipair>,
    rateModelPlans: RateModelPlan[],
    authority: PublicKey,
    futarchyAuthority: PublicKey,
): Promise<InstructionItem[][]> {
    const groups: InstructionItem[][] = [];

    for (const rateModelPlan of rateModelPlans) {
        const createIx = await program.methods
            .createRateModel({
                targetUtilStartBps: new BN(rateModelPlan.params.targetUtilStartBps),
                targetUtilEndBps: new BN(rateModelPlan.params.targetUtilEndBps),
                halfLifeMs: new BN(rateModelPlan.params.halfLifeMs),
                minRateBps: new BN(rateModelPlan.params.minRateBps),
                maxRateBps: new BN(rateModelPlan.params.maxRateBps),
                initialRateBps: new BN(rateModelPlan.params.initialRateBps),
            })
            .accountsPartial({
                authoritySigner: authority,
                futarchyAuthority,
                rateModel: rateModelPlan.keypair.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .instruction();

        const items: InstructionItem[] = [
            {
                kind: 'create-rate-model',
                ix: createIx,
                rateModel: rateModelPlan,
            },
        ];

        for (const pair of rateModelPlan.pairs) {
            const setIx = await program.methods
                .setPairRateModel()
                .accountsPartial({
                    authoritySigner: authority,
                    futarchyAuthority,
                    pair: pair.pair,
                    newRateModel: rateModelPlan.keypair.publicKey,
                    systemProgram: SystemProgram.programId,
                })
                .instruction();

            items.push({
                kind: 'set-pair-rate-model',
                ix: setIx,
                rateModel: rateModelPlan,
                pair,
            });
        }

        groups.push(items);
    }

    return groups;
}

function packTransactions(
    groups: InstructionItem[][],
    feePayer: PublicKey,
    recentBlockhash: string,
    opts: CliOptions,
): BuiltTransaction[] {
    const built: BuiltTransaction[] = [];

    for (const group of groups) {
        const createItem = group[0];
        const setItems = group.slice(1);
        let isFirstForModel = true;
        let cursor = 0;

        while (cursor < setItems.length || isFirstForModel) {
            const batchItems: InstructionItem[] = isFirstForModel ? [createItem] : [];
            isFirstForModel = false;

            while (cursor < setItems.length) {
                const nextItems = [...batchItems, setItems[cursor]];
                if (opts.maxInstructionsPerTx && nextItems.length > opts.maxInstructionsPerTx) {
                    break;
                }

                const maybeTx = tryBuildTransaction(
                    nextItems,
                    feePayer,
                    recentBlockhash,
                    opts,
                    built.length + 1,
                );

                if (!maybeTx) {
                    if (batchItems.length === 0) {
                        throw new Error(
                            `A single set_pair_rate_model instruction for pair ${setItems[cursor].pair?.pair.toBase58()} exceeds the batch limit`,
                        );
                    }
                    break;
                }

                batchItems.push(setItems[cursor]);
                cursor += 1;
            }

            const tx = buildTransactionOrThrow(
                batchItems,
                feePayer,
                recentBlockhash,
                opts,
                built.length + 1,
            );
            built.push(tx);
        }
    }

    return built;
}

function tryBuildTransaction(
    items: InstructionItem[],
    feePayer: PublicKey,
    recentBlockhash: string,
    opts: CliOptions,
    index: number,
): BuiltTransaction | null {
    try {
        const tx = buildVersionedTransaction(items, feePayer, recentBlockhash, opts.signRateModels);
        const bytes = tx.serialize();
        if (bytes.length > opts.batchByteLimit) {
            return null;
        }
        return describeBuiltTransaction(index, items, bytes);
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        if (message.includes('encoding overruns Uint8Array') || message.includes('Transaction too large')) {
            return null;
        }
        throw error;
    }
}

function buildTransactionOrThrow(
    items: InstructionItem[],
    feePayer: PublicKey,
    recentBlockhash: string,
    opts: CliOptions,
    index: number,
): BuiltTransaction {
    const built = tryBuildTransaction(items, feePayer, recentBlockhash, opts, index);
    if (!built) {
        const names = items.map((item) =>
            item.kind === 'create-rate-model'
                ? `create:${item.rateModel.keypair.publicKey.toBase58()}`
                : `set:${item.pair?.pair.toBase58()}`,
        );
        throw new Error(
            `Cannot fit batch within ${opts.batchByteLimit} bytes: ${names.join(', ')}`,
        );
    }
    return built;
}

function buildVersionedTransaction(
    items: InstructionItem[],
    feePayer: PublicKey,
    recentBlockhash: string,
    signRateModels: boolean,
): VersionedTransaction {
    const message = new TransactionMessage({
        payerKey: feePayer,
        recentBlockhash,
        instructions: items.map((item) => item.ix),
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    if (signRateModels) {
        const signers = uniqueRateModelSigners(items);
        if (signers.length > 0) {
            tx.sign(signers);
        }
    }
    return tx;
}

function uniqueRateModelSigners(items: InstructionItem[]): Keypair[] {
    const signersByPubkey = new Map<string, Keypair>();
    for (const item of items) {
        if (item.kind !== 'create-rate-model') {
            continue;
        }
        signersByPubkey.set(item.rateModel.keypair.publicKey.toBase58(), item.rateModel.keypair);
    }
    return [...signersByPubkey.values()];
}

function describeBuiltTransaction(
    index: number,
    items: InstructionItem[],
    bytes: Uint8Array,
): BuiltTransaction {
    return {
        index,
        base64: Buffer.from(bytes).toString('base64'),
        byteLength: bytes.length,
        instructionCount: items.length,
        createRateModels: items
            .filter((item) => item.kind === 'create-rate-model')
            .map((item) => item.rateModel.keypair.publicKey.toBase58()),
        setPairs: items
            .filter((item) => item.kind === 'set-pair-rate-model' && item.pair)
            .map((item) => item.pair!.pair.toBase58()),
    };
}

async function resolveAuthority(
    opts: CliOptions,
    program: Program<Omnipair>,
): Promise<{ futarchyAuthority: PublicKey; authority: PublicKey }> {
    const [futarchyAuthority] = PublicKey.findProgramAddressSync(
        [Buffer.from('futarchy_authority')],
        program.programId,
    );

    if (opts.authority) {
        return { futarchyAuthority, authority: new PublicKey(opts.authority) };
    }

    const futarchy = await program.account.futarchyAuthority.fetch(futarchyAuthority);
    return {
        futarchyAuthority,
        authority: new PublicKey(futarchy.authority),
    };
}

function writeOutputs(
    outDir: string,
    rateModelPlans: RateModelPlan[],
    transactions: BuiltTransaction[],
    opts: CliOptions,
    context: {
        rpcUrl: string;
        programId: PublicKey;
        authority: PublicKey;
        feePayer: PublicKey;
        futarchyAuthority: PublicKey;
        recentBlockhash: string;
    },
): void {
    fs.mkdirSync(outDir, { recursive: true });

    const keypairEntries = rateModelPlans.map((plan) => {
        const filename = `${plan.keypair.publicKey.toBase58()}-keypair.json`;
        const keypairPath = path.join(outDir, filename);
        if (opts.writeKeypairs) {
            fs.writeFileSync(
                keypairPath,
                JSON.stringify(Array.from(plan.keypair.secretKey)),
                { mode: 0o600 },
            );
        }
        return {
            rateModel: plan.keypair.publicKey.toBase58(),
            keypairPath: opts.writeKeypairs ? keypairPath : null,
            params: plan.params,
            pairs: plan.pairs.map((pair) => ({
                pair: pair.pair.toBase58(),
                currentRateModel: pair.currentRateModel.toBase58(),
                currentParams: pair.currentParams,
                db: pair.db ?? null,
            })),
        };
    });

    const manifest = {
        generatedAt: new Date().toISOString(),
        rpcUrl: redactUrl(context.rpcUrl),
        programId: context.programId.toBase58(),
        authority: context.authority.toBase58(),
        feePayer: context.feePayer.toBase58(),
        futarchyAuthority: context.futarchyAuthority.toBase58(),
        recentBlockhash: context.recentBlockhash,
        targetOverrides: TARGET_RATE_MODEL,
        shareRateModels: opts.shareRateModels,
        signRateModels: opts.signRateModels,
        writeKeypairs: opts.writeKeypairs,
        rateModels: keypairEntries,
        transactions,
        notes: [
            'Transactions are built for import/proposal workflows and are not submitted by this script.',
            'If used as raw Solana transactions, the recent blockhash expires quickly.',
            'create_rate_model requires each generated RateModel account to be a signer; keep the keypair files for execution flows that need those signatures.',
        ],
    };

    fs.writeFileSync(
        path.join(outDir, 'manifest.json'),
        JSON.stringify(manifest, null, 2),
    );

    fs.writeFileSync(
        path.join(outDir, 'transactions.base64.txt'),
        transactions.map((tx) => tx.base64).join('\n') + '\n',
    );

    fs.writeFileSync(
        path.join(outDir, 'summary.md'),
        renderSummary(rateModelPlans, transactions, context),
    );
}

function renderSummary(
    rateModelPlans: RateModelPlan[],
    transactions: BuiltTransaction[],
    context: {
        programId: PublicKey;
        authority: PublicKey;
        feePayer: PublicKey;
        futarchyAuthority: PublicKey;
        recentBlockhash: string;
    },
): string {
    const lines = [
        '# IRC Upgrade Transaction Summary',
        '',
        `Program: ${context.programId.toBase58()}`,
        `Authority signer: ${context.authority.toBase58()}`,
        `Fee payer: ${context.feePayer.toBase58()}`,
        `Futarchy authority PDA: ${context.futarchyAuthority.toBase58()}`,
        `Recent blockhash: ${context.recentBlockhash}`,
        '',
        `Target params: util ${TARGET_RATE_MODEL.targetUtilStartBps}-${TARGET_RATE_MODEL.targetUtilEndBps} bps, half-life ${TARGET_RATE_MODEL.halfLifeMs} ms, min rate ${TARGET_RATE_MODEL.minRateBps} bps.`,
        `Rate model accounts: ${rateModelPlans.length}`,
        `Pairs to update: ${rateModelPlans.reduce((sum, plan) => sum + plan.pairs.length, 0)}`,
        `Transactions: ${transactions.length}`,
        '',
        '## Transactions',
        '',
    ];

    for (const tx of transactions) {
        lines.push(
            `- ${tx.index}. ${tx.instructionCount} ix, ${tx.byteLength} bytes, creates ${tx.createRateModels.length}, sets ${tx.setPairs.length}`,
        );
    }

    lines.push('', '## Base64', '');
    for (const tx of transactions) {
        lines.push(`### Transaction ${tx.index}`, '', tx.base64, '');
    }

    return lines.join('\n');
}

async function mapWithConcurrency<T, U>(
    items: T[],
    concurrency: number,
    mapper: (item: T, index: number) => Promise<U>,
): Promise<U[]> {
    const results = new Array<U>(items.length);
    let next = 0;

    async function worker(): Promise<void> {
        while (next < items.length) {
            const index = next;
            next += 1;
            results[index] = await mapper(items[index], index);
        }
    }

    const workerCount = Math.min(concurrency, Math.max(items.length, 1));
    await Promise.all(Array.from({ length: workerCount }, () => worker()));
    return results;
}

function timestampForPath(): string {
    return new Date().toISOString().replace(/[:.]/g, '-');
}

function redactUrl(rawUrl: string): string {
    try {
        const url = new URL(rawUrl);
        if (url.username) {
            url.username = '***';
        }
        if (url.password) {
            url.password = '***';
        }
        for (const key of [...url.searchParams.keys()]) {
            if (/(api[-_]?key|token|secret|password|auth|key)/i.test(key)) {
                url.searchParams.set(key, '***');
            }
        }
        return url.toString();
    } catch {
        return rawUrl.replace(/(api[-_]?key|token|secret|password|auth|key)=([^&\s]+)/gi, '$1=***');
    }
}

async function main(): Promise<void> {
    const opts = parseArgs(process.argv.slice(2));
    loadEnv(opts);

    const rpcUrl = getRpcUrl(opts);
    const connection = new Connection(rpcUrl, 'confirmed');
    const programId = getProgramId(opts);
    const placeholderWallet = opts.feePayer
        ? new PublicKey(opts.feePayer)
        : opts.authority
            ? new PublicKey(opts.authority)
            : programId;
    const provider = makeProvider(connection, placeholderWallet);
    const program = new Program<Omnipair>({ ...(idl as Omnipair), address: programId.toBase58() }, provider);

    console.log(`Source: ${opts.source}${opts.source === 'db' && opts.visibleOnly ? ' (visible pools only)' : ''}`);
    console.log(`RPC: ${redactUrl(rpcUrl)}`);
    console.log(`Program: ${program.programId.toBase58()}`);

    const { futarchyAuthority, authority } = await resolveAuthority(opts, program);
    const feePayer = opts.feePayer ? new PublicKey(opts.feePayer) : authority;
    console.log(`Futarchy authority PDA: ${futarchyAuthority.toBase58()}`);
    console.log(`Authority signer: ${authority.toBase58()}`);
    console.log(`Fee payer: ${feePayer.toBase58()}`);

    const candidates = await loadCandidates(opts, program);
    console.log(`Candidate pairs: ${candidates.length}`);

    const pairPlans = await buildPairPlans(
        candidates,
        program,
        opts.includeAlreadyTarget,
        opts.concurrency,
    );
    console.log(`Pairs needing upgrade: ${pairPlans.length}`);

    if (pairPlans.length === 0) {
        console.log('Nothing to do. All selected pairs already match the target params.');
        return;
    }

    const rateModelPlans = groupRateModels(pairPlans, opts.shareRateModels);
    console.log(`New RateModel accounts: ${rateModelPlans.length}`);

    const instructionGroups = await createInstructionItems(
        program,
        rateModelPlans,
        authority,
        futarchyAuthority,
    );
    const { blockhash } = await connection.getLatestBlockhash('confirmed');
    const transactions = packTransactions(instructionGroups, feePayer, blockhash, opts);
    console.log(`Built transactions: ${transactions.length}`);
    transactions.forEach((tx) => {
        console.log(
            `  ${tx.index}: ${tx.instructionCount} ix, ${tx.byteLength} bytes, creates=${tx.createRateModels.length}, sets=${tx.setPairs.length}`,
        );
    });

    if (opts.printBase64) {
        console.log('\nBase64 transactions:');
        transactions.forEach((tx) => {
            console.log(`${tx.index}: ${tx.base64}`);
        });
    }

    if (opts.dryRun) {
        console.log('Dry run complete; no files written.');
        return;
    }

    const outDir = path.resolve(
        opts.outDir || path.join(getRepoRoot(), '.generated', `irc-upgrade-${timestampForPath()}`),
    );
    writeOutputs(outDir, rateModelPlans, transactions, opts, {
        rpcUrl,
        programId: program.programId,
        authority,
        feePayer,
        futarchyAuthority,
        recentBlockhash: blockhash,
    });

    console.log(`Wrote manifest and base64 payloads to ${outDir}`);
}

main().catch((error) => {
    console.error(error instanceof Error ? error.stack || error.message : error);
    process.exit(1);
});
