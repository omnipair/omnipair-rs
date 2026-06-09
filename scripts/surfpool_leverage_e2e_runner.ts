import { spawn } from 'node:child_process';
import { Connection } from '@solana/web3.js';

const SURFPOOL_URL = process.env.SURFPOOL_RPC_URL ?? process.env.ANCHOR_PROVIDER_URL ?? 'http://127.0.0.1:8899';
const SURFPOOL_ARGS = [
    'start',
    '--network',
    'mainnet',
    '--no-tui',
    '--no-studio',
    '--yes',
    '--legacy-anchor-compatibility',
    '--airdrop-keypair-path',
    process.env.ANCHOR_WALLET ?? 'deployer-keypair.json',
    '--artifacts-path',
    'target/deploy',
    '--log-path',
    '/tmp/omnipair-surfpool-logs',
];

async function sleep(ms: number) {
    await new Promise((resolve) => setTimeout(resolve, ms));
}

async function rpcReachable() {
    try {
        await new Connection(SURFPOOL_URL, 'confirmed').getVersion();
        return true;
    } catch {
        return false;
    }
}

async function waitForRpc(childExited: () => boolean) {
    for (let attempt = 0; attempt < 60; attempt += 1) {
        if (childExited()) {
            throw new Error('Surfpool exited before the RPC became reachable');
        }
        if (await rpcReachable()) {
            await sleep(5_000);
            return;
        }
        await sleep(1_000);
    }
    throw new Error(`Surfpool RPC is not reachable at ${SURFPOOL_URL}`);
}

function runTest(): Promise<number> {
    return new Promise((resolve) => {
        const child = spawn('npm', ['run', 'test-surfpool-leverage'], {
            cwd: process.cwd(),
            env: process.env,
            stdio: 'inherit',
        });
        child.on('exit', (code) => resolve(code ?? 1));
    });
}

function stopSurfpool(
    surfpool: ReturnType<typeof spawn>,
    childExited: () => boolean,
): Promise<void> {
    return new Promise((resolve) => {
        if (childExited()) {
            resolve();
            return;
        }

        const forceKill = setTimeout(() => {
            if (!childExited()) {
                surfpool.kill('SIGKILL');
            }
        }, 3_000);

        surfpool.once('exit', () => {
            clearTimeout(forceKill);
            resolve();
        });
        surfpool.kill('SIGINT');
    });
}

async function main() {
    if (await rpcReachable()) {
        throw new Error(
            `Surfpool RPC is already reachable at ${SURFPOOL_URL}. Stop it first, or run npm run test-surfpool-leverage directly.`,
        );
    }

    const surfpool = spawn('surfpool', SURFPOOL_ARGS, {
        cwd: process.cwd(),
        env: process.env,
        stdio: 'inherit',
    });
    let exited = false;
    surfpool.on('exit', () => {
        exited = true;
    });

    try {
        await waitForRpc(() => exited);
        const testExitCode = await runTest();
        await stopSurfpool(surfpool, () => exited);
        process.exit(testExitCode);
    } catch (error) {
        await stopSurfpool(surfpool, () => exited);
        throw error;
    }
}

main().catch((error) => {
    console.error(error);
    process.exit(1);
});
