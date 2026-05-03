import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();
const WALLET = process.env.ANCHOR_WALLET ?? 'deployer-keypair.json';
const CLUSTER = process.env.DEVNET_RPC_URL ?? process.env.DEVNET_CLUSTER ?? 'devnet';
const PROGRAMS = ['omnipair', 'leverage_delegate'];

function run(command: string, args: string[]) {
    console.log(`\n$ ${command} ${args.join(' ')}`);
    const result = spawnSync(command, args, {
        cwd: ROOT,
        env: process.env,
        stdio: 'inherit',
    });
    if (result.status !== 0) {
        throw new Error(`${command} ${args.join(' ')} failed with status ${result.status}`);
    }
}

function readProgramId(programName: string): string {
    const idlPath = join(ROOT, 'target', 'idl', `${programName}.json`);
    if (!existsSync(idlPath)) {
        return '<missing idl>';
    }
    return JSON.parse(readFileSync(idlPath, 'utf8')).address;
}

async function main() {
    if (!existsSync(join(ROOT, WALLET))) {
        throw new Error(`Wallet not found at ${WALLET}. Set ANCHOR_WALLET or add deployer-keypair.json.`);
    }

    console.log('Devnet leverage deployment');
    console.log('Cluster:', CLUSTER);
    console.log('Wallet:', WALLET);

    run('solana', ['balance', '-k', WALLET, '--url', CLUSTER]);

    for (const programName of PROGRAMS) {
        run('anchor', [
            'build',
            '-p',
            programName,
            '--provider.cluster',
            CLUSTER,
            '--',
            '--features',
            'development',
        ]);
    }

    for (const programName of PROGRAMS) {
        run('anchor', [
            'deploy',
            '-p',
            programName,
            '--provider.cluster',
            CLUSTER,
            '--provider.wallet',
            WALLET,
        ]);

        const programId = readProgramId(programName);
        console.log(`${programName}: ${programId}`);
        if (programId !== '<missing idl>') {
            run('solana', ['program', 'show', programId, '--url', CLUSTER]);
        }
    }

    console.log('\nDeployment complete. Run `npm run test-devnet-leverage` for the e2e smoke test.');
}

main().catch((error) => {
    console.error(error);
    process.exit(1);
});
