import http from 'node:http';

const PORT = Number(process.env.PORT ?? process.env.FORK_RPC_PROXY_PORT ?? 8898);
const TARGET_RPC_URL = process.env.SURFPOOL_RPC_URL ?? 'http://127.0.0.1:8899';
const ADMIN_TOKEN = process.env.FORK_ADMIN_TOKEN ?? '';

const BLOCKED_METHOD_PREFIXES = ['surfnet_'];
const EXTRA_BLOCKED_METHODS = new Set(
    (process.env.FORK_RPC_PROXY_BLOCKED_METHODS ?? '')
        .split(',')
        .map((method) => method.trim())
        .filter(Boolean),
);

function corsHeaders() {
    return {
        'access-control-allow-origin': process.env.FORK_RPC_PROXY_CORS_ORIGIN ?? '*',
        'access-control-allow-methods': 'POST, OPTIONS, GET',
        'access-control-allow-headers': 'content-type, authorization, x-fork-admin-token',
    };
}

function isAdmin(req: http.IncomingMessage): boolean {
    if (!ADMIN_TOKEN) {
        return false;
    }
    const auth = req.headers.authorization;
    const headerToken = req.headers['x-fork-admin-token'];
    return auth === `Bearer ${ADMIN_TOKEN}` || headerToken === ADMIN_TOKEN;
}

function methodIsBlocked(method: string): boolean {
    return (
        EXTRA_BLOCKED_METHODS.has(method) ||
        BLOCKED_METHOD_PREFIXES.some((prefix) => method.startsWith(prefix))
    );
}

function extractMethods(payload: unknown): string[] {
    const items = Array.isArray(payload) ? payload : [payload];
    return items
        .map((item) => {
            if (item && typeof item === 'object' && 'method' in item) {
                return String((item as { method: unknown }).method);
            }
            return '';
        })
        .filter(Boolean);
}

async function readBody(req: http.IncomingMessage): Promise<string> {
    const chunks: Buffer[] = [];
    for await (const chunk of req) {
        chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
    }
    return Buffer.concat(chunks).toString('utf8');
}

function sendJson(res: http.ServerResponse, status: number, value: unknown) {
    res.writeHead(status, {
        'content-type': 'application/json',
        ...corsHeaders(),
    });
    res.end(JSON.stringify(value));
}

const server = http.createServer(async (req, res) => {
    if (req.method === 'OPTIONS') {
        res.writeHead(204, corsHeaders());
        res.end();
        return;
    }

    if (req.method === 'GET' && req.url === '/health') {
        sendJson(res, 200, { ok: true, target: TARGET_RPC_URL });
        return;
    }

    if (req.method !== 'POST') {
        sendJson(res, 405, { error: 'method_not_allowed' });
        return;
    }

    try {
        const body = await readBody(req);
        const payload = JSON.parse(body);
        const blocked = extractMethods(payload).filter(methodIsBlocked);

        if (blocked.length > 0 && !isAdmin(req)) {
            sendJson(res, 403, {
                jsonrpc: '2.0',
                error: {
                    code: -32099,
                    message: `Fork cheatcode RPC methods are blocked by the public proxy: ${blocked.join(', ')}`,
                },
                id: Array.isArray(payload) ? null : payload?.id ?? null,
            });
            return;
        }

        const upstream = await fetch(TARGET_RPC_URL, {
            method: 'POST',
            headers: {
                'content-type': 'application/json',
            },
            body,
        });

        const text = await upstream.text();
        res.writeHead(upstream.status, {
            'content-type': upstream.headers.get('content-type') ?? 'application/json',
            ...corsHeaders(),
        });
        res.end(text);
    } catch (error) {
        sendJson(res, 500, {
            jsonrpc: '2.0',
            error: {
                code: -32603,
                message: error instanceof Error ? error.message : String(error),
            },
            id: null,
        });
    }
});

server.listen(PORT, '0.0.0.0', () => {
    console.log(`fork RPC proxy listening on :${PORT}, target ${TARGET_RPC_URL}`);
});
