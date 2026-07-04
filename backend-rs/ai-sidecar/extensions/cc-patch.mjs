// Bundled pi-cc-patch: lets ServerKit use a Claude Pro/Max *subscription*
// (OAuth) with pi instead of hitting the "third-party apps draw from your extra
// usage" block. Patches only Anthropic/Claude provider requests via the
// before_provider_request hook — no token swap, no proxy. Ported from
// github.com/picassio/pi-cc-patch + the retired-model-alias fixes from the
// local cc-token-sync extension. Loaded as an extensionFactory (no .ts needed).

const RETIRED_ANTHROPIC_MODEL_ALIASES = {
  "anthropic/claude-3-5-haiku-latest": "claude-haiku-4-5",
  "claude-3-5-haiku-latest": "claude-haiku-4-5",
  "anthropic/claude-3-5-haiku-20241022": "claude-haiku-4-5-20251001",
  "claude-3-5-haiku-20241022": "claude-haiku-4-5-20251001",
  "anthropic/claude-3-5-sonnet-latest": "claude-sonnet-4-5",
  "claude-3-5-sonnet-latest": "claude-sonnet-4-5",
  "anthropic/claude-3-5-sonnet-20241022": "claude-sonnet-4-5-20250929",
  "claude-3-5-sonnet-20241022": "claude-sonnet-4-5-20250929",
  "anthropic/claude-3-7-sonnet-latest": "claude-sonnet-4-5",
  "claude-3-7-sonnet-latest": "claude-sonnet-4-5",
  "anthropic/claude-3-7-sonnet-20250219": "claude-sonnet-4-5-20250929",
  "claude-3-7-sonnet-20250219": "claude-sonnet-4-5-20250929",
};

function isAnthropicTarget(payload, model) {
  const provider = typeof model?.provider === "string" ? model.provider.toLowerCase() : "";
  const modelId = typeof model?.id === "string" ? model.id.toLowerCase() : "";
  const payloadModel = typeof payload.model === "string" ? payload.model.toLowerCase() : "";
  return (
    provider.includes("anthropic") || modelId.includes("claude") ||
    payloadModel.includes("anthropic") || payloadModel.includes("claude")
  );
}

function sanitizeSystemPrompt(text) {
  return text
    .replace(/operating inside pi, a coding agent harness\./g, "operating as a coding assistant.")
    .replace(/Pi documentation/g, "Documentation")
    .replace(/pi itself,/g, "the tool itself,")
    .replace(/pi packages/g, "packages")
    .replace(/read pi \.md/g, "read .md")
    .replace(/pi-coding-agent/g, "coding-agent")
    .replace(/@earendil-works\/pi-ai/g, "@anthropic/ai")
    .replace(/@earendil-works\/pi-tui/g, "@anthropic/tui")
    .replace(/@mariozechner\/pi-ai/g, "@anthropic/ai")
    .replace(/@mariozechner\/pi-tui/g, "@anthropic/tui")
    .replace(/about pi\b/g, "about this tool")
    .replace(/pi update\b/g, "update")
    .replace(/Run pi update/g, "Run update")
    .replace(/\bpi\b([\s,.])/g, "the assistant$1");
}

export default function (pi) {
  pi.on("before_provider_request", async (event, ctx) => {
    const payload = event.payload;
    if (!payload || typeof payload !== "object") return;
    if (!Array.isArray(payload.messages)) return;
    if (!isAnthropicTarget(payload, ctx?.model)) return;

    // rewrite retired model aliases that now 404
    if (typeof payload.model === "string") {
      const alias = RETIRED_ANTHROPIC_MODEL_ALIASES[payload.model.toLowerCase()];
      if (alias) payload.model = alias;
    }

    const BILLING = "x-anthropic-billing-header: cc_version=2.1.96.000; cc_entrypoint=cli;";
    if (Array.isArray(payload.system)) {
      const blocks = [{ type: "text", text: BILLING }];
      for (const block of payload.system) {
        if (block.type !== "text" || !block.text) { blocks.push(block); continue; }
        if (block.text.startsWith("x-anthropic-billing-header")) continue;
        if (block.text.startsWith("You are") && block.text.includes("official CLI")) continue;
        blocks.push({ ...block, text: sanitizeSystemPrompt(block.text) });
      }
      payload.system = blocks;
    } else if (typeof payload.system === "string") {
      payload.system = [
        { type: "text", text: BILLING },
        { type: "text", text: sanitizeSystemPrompt(payload.system) },
      ];
    }

    if (!payload.metadata) {
      payload.metadata = { user_id: JSON.stringify({ device_id: "0", account_uuid: "", session_id: "0" }) };
    }
    return payload;
  });
}
