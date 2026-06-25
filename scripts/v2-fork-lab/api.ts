import http from "node:http";

const PORT = Number(process.env.PORT ?? process.env.FORK_API_PORT ?? 8080);
const SURFPOOL_RPC_URL = process.env.SURFPOOL_RPC_URL ?? "http://127.0.0.1:8899";
const PUBLIC_RPC_URL =
  process.env.PUBLIC_SURFPOOL_RPC_URL ?? process.env.SURFPOOL_RPC_PROXY_URL ?? SURFPOOL_RPC_URL;

function corsHeaders() {
  return {
    "access-control-allow-origin": process.env.FORK_API_CORS_ORIGIN ?? "*",
    "access-control-allow-methods": "GET, POST, OPTIONS",
    "access-control-allow-headers":
      "content-type, authorization, solana-client, x-fork-admin-token",
  };
}

function replacer(_key: string, value: unknown): unknown {
  if (typeof value === "bigint") return value.toString();
  if (value && typeof value === "object") {
    const maybeBase58 = (value as { toBase58?: unknown }).toBase58;
    if (typeof maybeBase58 === "function") return maybeBase58.call(value);
    if (value.constructor?.name === "BN") return value.toString();
  }
  return value;
}

function sendJson(res: http.ServerResponse, status: number, value: unknown) {
  res.writeHead(status, {
    "content-type": "application/json",
    ...corsHeaders(),
  });
  res.end(JSON.stringify(value, replacer));
}

async function readBody(req: http.IncomingMessage): Promise<Record<string, unknown>> {
  const chunks: Buffer[] = [];
  for await (const chunk of req) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  const text = Buffer.concat(chunks).toString("utf8");
  return text ? JSON.parse(text) : {};
}

function healthPayload() {
  return {
    ok: true,
    rpcUrl: SURFPOOL_RPC_URL,
    publicRpcUrl: PUBLIC_RPC_URL,
    programId:
      process.env.OMNIPAIR_V2_PROGRAM_ID ??
      process.env.PROGRAM_ID_V2 ??
      "358bjJKXWxeAXAzteX1xTgyd9JNnjtzW8fnwCS8Da1mv",
  };
}

const server = http.createServer(async (req, res) => {
  if (req.method === "OPTIONS") {
    res.writeHead(204, corsHeaders());
    res.end();
    return;
  }

  if (req.method === "GET" && req.url === "/health") {
    sendJson(res, 200, healthPayload());
    return;
  }

  try {
    const body = req.method === "POST" ? await readBody(req) : {};
    const { route } = await import("./api_core.js");
    const value = await route(req, body);
    sendJson(res, 200, value);
  } catch (error) {
    sendJson(res, 400, {
      success: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }
});

server.listen(PORT, "0.0.0.0", () => {
  console.log(`V2 fork API listening on :${PORT}`);
  console.log(`Surfpool RPC: ${SURFPOOL_RPC_URL}`);
  console.log(`Public RPC: ${PUBLIC_RPC_URL}`);
  console.log("V2 fork runtime will initialize on first non-health request");
});
