import { Keypair, Connection, Transaction } from "@solana/web3.js";
import { localE2E } from "./api_core.js";

function apiBaseUrl(): string | null {
  const value = process.env.FORK_API_URL ?? process.env.V2_FORK_API_URL;
  return value ? value.replace(/\/$/, "") : null;
}

function loadE2eKeypair(): Keypair {
  const inline = process.env.FORK_E2E_KEYPAIR_JSON ?? process.env.FORK_E2E_KEYPAIR_BASE64;
  if (!inline) return Keypair.generate();
  const json = inline.trim().startsWith("[")
    ? inline
    : Buffer.from(inline, "base64").toString("utf8");
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(json) as number[]));
}

async function fetchJson(url: string, init?: RequestInit): Promise<any> {
  const response = await fetch(url, {
    ...init,
    headers: {
      "content-type": "application/json",
      ...(init?.headers ?? {}),
    },
  });
  const payload = await response.json();
  if (!response.ok || payload?.success === false) {
    throw new Error(`${init?.method ?? "GET"} ${url} failed: ${JSON.stringify(payload)}`);
  }
  return payload;
}

async function remoteE2E(baseUrl: string) {
  const configResponse = await fetchJson(`${baseUrl}/api/v2/fork/config`);
  const config = configResponse.data;
  const wallet = loadE2eKeypair();
  const connection = new Connection(config.rpcUrl, "confirmed");

  await fetchJson(`${baseUrl}/api/v2/fork/fund-wallet`, {
    method: "POST",
    body: JSON.stringify({
      wallet: wallet.publicKey.toBase58(),
      sol: Number(process.env.FORK_E2E_SOL ?? "10"),
      baseAmount: process.env.FORK_E2E_BASE_FUNDING ?? "100",
      quoteAmount: process.env.FORK_E2E_QUOTE_FUNDING ?? "100",
    }),
  });

  async function buildSignAndSend(path: string, body: Record<string, unknown>) {
    const response = await fetchJson(`${baseUrl}${path}`, {
      method: "POST",
      body: JSON.stringify({ owner: wallet.publicKey.toBase58(), ...body }),
    });
    const tx = Transaction.from(Buffer.from(response.data.transaction, "base64"));
    tx.sign(wallet);
    const signature = await connection.sendRawTransaction(tx.serialize());
    await connection.confirmTransaction(signature, "confirmed");
    return signature;
  }

  const addLiquiditySig = await buildSignAndSend("/api/v2/fork/tx/add-liquidity", {
    baseDepositAmount: process.env.FORK_E2E_LIQUIDITY_BASE ?? "1",
    quoteDepositAmount: process.env.FORK_E2E_LIQUIDITY_QUOTE ?? "1",
  });
  const swapSig = await buildSignAndSend("/api/v2/fork/tx/swap", {
    assetIn: "base",
    exactAssetIn: process.env.FORK_E2E_SWAP_IN ?? "0.1",
    minAssetOut: "0",
  });

  return {
    ok: true,
    mode: "remote-api",
    market: config.market,
    wallet: wallet.publicKey.toBase58(),
    addLiquiditySig,
    swapSig,
  };
}

export async function runSurfpoolV2E2E() {
  const baseUrl = apiBaseUrl();
  if (baseUrl) return remoteE2E(baseUrl);
  return localE2E();
}

runSurfpoolV2E2E()
  .then((result) => {
    console.log(JSON.stringify(result, null, 2));
  })
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
