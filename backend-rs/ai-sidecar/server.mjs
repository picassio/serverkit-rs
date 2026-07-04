// ServerKit AI sidecar — embeds the pi SDK (@earendil-works/pi-coding-agent)
// in-process and exposes an SSE chat endpoint the Rust backend proxies to.
// This is the "full native SDK" path: real AgentSession objects, incremental
// deltas, in-process custom tools, and long-lived per-conversation sessions.
import http from "node:http";
import { randomUUID } from "node:crypto";
import {
  createAgentSession,
  DefaultResourceLoader,
  AuthStorage,
  ModelRegistry,
  SessionManager,
  getAgentDir,
} from "@earendil-works/pi-coding-agent";
import { makeTools } from "./tools.mjs";
import ccPatch from "./extensions/cc-patch.mjs";

const PORT = Number(process.env.SK_SIDECAR_PORT || 5056);
const SECRET = process.env.SK_SIDECAR_TOKEN || "";
const SDK_VERSION = "0.79.6";

const SYSTEM_PROMPT =
  "You are the ServerKit assistant, embedded in a server control panel that manages web apps, " +
  "databases, Docker, nginx, PHP, and Magento stores. Answer concisely and practically. When " +
  "ServerKit tools (names prefixed sk_) are available, use them to read live state and perform " +
  "the operator's requested actions (create/manage Magento stores and websites, run Magento " +
  "actions, control containers, back up databases, install templates). Prefer a read tool to " +
  "confirm state before and after a write. Be careful with destructive actions and confirm " +
  "intent in your reply. When no tools are available, give guidance the operator can act on.";

const authStorage = AuthStorage.create();
const modelRegistry = ModelRegistry.create(authStorage);

// conversation_id -> { session, ctx, toolsEnabled, modelKey }
const pool = new Map();

// Resolve a "provider/id" (or bare id) to a Model, or undefined for default.
function resolveModel(spec) {
  if (!spec) return undefined;
  const [provider, ...rest] = String(spec).split("/");
  const id = rest.join("/") || provider;
  try {
    return modelRegistry.find(rest.length ? provider : "anthropic", id) || modelRegistry.find(provider, id) || undefined;
  } catch {
    return undefined;
  }
}

async function getEntry(convId, toolsEnabled, ctx, modelKey) {
  const existing = pool.get(convId);
  if (existing && existing.toolsEnabled === toolsEnabled && existing.modelKey === (modelKey || "")) {
    existing.ctx.apiUrl = ctx.apiUrl;
    existing.ctx.apiToken = ctx.apiToken;
    return existing;
  }
  if (existing) existing.session.dispose?.();

  const liveCtx = { apiUrl: ctx.apiUrl, apiToken: ctx.apiToken };
  const loader = new DefaultResourceLoader({
    cwd: process.cwd(),
    agentDir: getAgentDir(),
    systemPromptOverride: () => SYSTEM_PROMPT,
    // bundled Claude Pro/Max subscription patch (anthropic-only)
    extensionFactories: [ccPatch],
  });
  await loader.reload();
  const opts = {
    resourceLoader: loader,
    sessionManager: SessionManager.inMemory(),
    authStorage,
    modelRegistry,
  };
  const model = resolveModel(modelKey);
  if (model) opts.model = model;
  if (toolsEnabled) opts.customTools = makeTools(liveCtx);
  else opts.noTools = "all";
  const { session } = await createAgentSession(opts);

  const entry = { session, ctx: liveCtx, toolsEnabled, modelKey: modelKey || "" };
  pool.set(convId, entry);
  return entry;
}

// pending OAuth logins: login_id -> { provider, url, instructions, manualResolve, done, error }
// Only one may be in flight at a time — the Anthropic flow binds a fixed
// localhost callback port (53692), so concurrent logins would collide.
const logins = new Map();
let activeLogin = null;

function sendJson(res, obj, status = 200) {
  res.writeHead(status, { "content-type": "application/json" });
  res.end(JSON.stringify(obj));
}

function sse(res, event, data) {
  res.write(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`);
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    let b = "";
    req.on("data", (c) => (b += c));
    req.on("end", () => { try { resolve(b ? JSON.parse(b) : {}); } catch (e) { reject(e); } });
    req.on("error", reject);
  });
}

const server = http.createServer(async (req, res) => {
  if (SECRET && req.headers["x-sk-sidecar-token"] !== SECRET) {
    res.writeHead(401).end("unauthorized");
    return;
  }
  if (req.method === "GET" && req.url === "/health") {
    sendJson(res, { ok: true, sdk: SDK_VERSION, sessions: pool.size });
    return;
  }

  // ── provider auth (login flow surfaced in the ServerKit web UI) ──────
  if (req.method === "GET" && req.url === "/auth/status") {
    const providers = authStorage.getOAuthProviders().map((p) => ({
      id: p.id, name: p.name, ...authStorage.getAuthStatus(p.id),
    }));
    sendJson(res, { providers });
    return;
  }
  if (req.method === "POST" && req.url === "/auth/login/start") {
    let body; try { body = await readBody(req); } catch { return sendJson(res, { error: "bad json" }, 400); }
    const provider = body.provider || "anthropic";
    if (activeLogin && logins.has(activeLogin)) {
      return sendJson(res, { error: "Another login is already in progress. Complete or cancel it first." }, 409);
    }
    const id = randomUUID();
    activeLogin = id;
    let urlResolve; const urlP = new Promise((r) => (urlResolve = r));
    let manualResolve; const manualP = new Promise((r) => (manualResolve = r));
    const entry = { provider, url: null, instructions: null, manualResolve, error: null };
    entry.done = authStorage
      .login(provider, {
        onAuth: ({ url, instructions }) => { entry.url = url; entry.instructions = instructions; urlResolve(); },
        onManualCodeInput: () => manualP,
        onPrompt: () => manualP,
        onProgress: () => {},
      })
      .then(() => {})
      .catch((e) => { entry.error = String(e?.message || e); urlResolve(); })
      .finally(() => { if (activeLogin === id) activeLogin = null; });
    logins.set(id, entry);
    await Promise.race([urlP, new Promise((r) => setTimeout(r, 15000))]);
    if (entry.error && !entry.url) { logins.delete(id); return sendJson(res, { error: entry.error }, 400); }
    sendJson(res, { login_id: id, provider, url: entry.url, instructions: entry.instructions });
    return;
  }
  if (req.method === "POST" && req.url === "/auth/login/complete") {
    let body; try { body = await readBody(req); } catch { return sendJson(res, { error: "bad json" }, 400); }
    const entry = logins.get(body.login_id);
    if (!entry) return sendJson(res, { error: "unknown login_id" }, 404);
    entry.manualResolve(String(body.input || ""));
    await entry.done;
    logins.delete(body.login_id);
    if (activeLogin === body.login_id) activeLogin = null;
    if (entry.error) return sendJson(res, { error: entry.error }, 400);
    sendJson(res, { ok: true, provider: entry.provider, status: authStorage.getAuthStatus(entry.provider) });
    return;
  }
  if (req.method === "POST" && req.url === "/auth/login/cancel") {
    let body; try { body = await readBody(req); } catch { return sendJson(res, { error: "bad json" }, 400); }
    const entry = logins.get(body.login_id);
    if (entry) { try { entry.manualResolve(""); } catch { /* ignore */ } logins.delete(body.login_id); }
    if (activeLogin === body.login_id) activeLogin = null;
    sendJson(res, { ok: true });
    return;
  }
  if (req.method === "POST" && req.url === "/auth/logout") {
    let body; try { body = await readBody(req); } catch { return sendJson(res, { error: "bad json" }, 400); }
    if (body.provider) authStorage.logout(body.provider);
    sendJson(res, { ok: true });
    return;
  }
  if (req.method === "POST" && req.url === "/chat/stream") {
    let body;
    try { body = await readBody(req); } catch { res.writeHead(400).end("bad json"); return; }
    const { conversation_id, message, tools_enabled = true, api_url, api_token, model } = body;
    if (!conversation_id || !message) { res.writeHead(400).end("missing fields"); return; }

    res.writeHead(200, {
      "content-type": "text/event-stream",
      "cache-control": "no-cache",
      connection: "keep-alive",
    });
    sse(res, "open", { conversation_id });

    let unsub = () => {};
    try {
      const entry = await getEntry(conversation_id, !!tools_enabled, { apiUrl: api_url, apiToken: api_token }, model);
      unsub = entry.session.subscribe((ev) => {
        try {
          if (ev.type === "message_update") {
            const a = ev.assistantMessageEvent;
            if (a?.type === "text_delta" && a.delta) sse(res, "text_delta", { text: a.delta });
            else if (a?.type === "thinking_delta" && a.delta) sse(res, "thinking_delta", { text: a.delta });
          } else if (ev.type === "tool_execution_start") {
            sse(res, "tool_use_start", { id: ev.toolCallId, name: ev.toolName });
            sse(res, "tool_use_stop", { id: ev.toolCallId, name: ev.toolName, input: ev.args ?? ev.input ?? {} });
          } else if (ev.type === "tool_execution_end") {
            const out = (ev.result?.content || []).filter((c) => c.type === "text").map((c) => c.text).join("");
            sse(res, "tool_result", { id: ev.toolCallId, output: out, is_error: !!ev.isError });
          }
        } catch { /* ignore per-event render errors */ }
      });
      await entry.session.prompt(String(message));
      sse(res, "done", { conversation_id });
    } catch (e) {
      sse(res, "error", { message: String(e?.message || e) });
    } finally {
      unsub();
      res.end();
    }
    return;
  }
  res.writeHead(404).end("not found");
});

server.listen(PORT, "127.0.0.1", () => {
  console.log(`[sk-ai-sidecar] pi SDK ${SDK_VERSION} listening on http://127.0.0.1:${PORT}`);
});
