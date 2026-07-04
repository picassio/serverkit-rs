# ADR 0002 — Third-party extension sandboxing (research note)

**Status:** Research note (not scheduled) · Task #42 of the Extensions Platform plan
**TL;DR:** in-process execution + declared-permission gate + curated registry is
the accepted risk posture. Out-of-process sandboxing is documented here as the
escalation path if third-party volume ever demands it — not built now.

## Current posture (what we actually ship)

A plugin's backend runs **in-process**: its blueprint is imported and registered
into the panel's Flask app, with the panel's privileges. Safety today comes from:

- **Curated registry** — entries are PR-reviewed (permissions honesty, checksum,
  license). No open publish.
- **Checksum-verified install** — the downloaded zip must match the index `sha256`.
- **Declared-permission gate** — `require_permission(slug, cap)` raises for
  undeclared capabilities (ADR context: this is *declaration-based*, not a syscall
  jail — an in-process plugin that bypasses the SDK and imports a host module
  directly is not stopped).
- **Gated pip** — `SERVERKIT_ALLOW_PLUGIN_PIP` is off by default, so a plugin's
  Python deps aren't installed (and setup.py can't run) without operator opt-in.
- **Status guard** — disabled ⇒ HTTP 503 + socket disconnect + paused jobs.

This is appropriate for first-party + a small, curated third-party set. It is
**not** a security boundary against a malicious in-process plugin.

## When to escalate

Escalate to real isolation when any of these become true: an open/self-serve
registry, untrusted publishers at volume, or multi-tenant panels where one
operator's plugin must not reach another's data.

## Options for out-of-process execution

1. **Subprocess + RPC.** Run each plugin's backend in its own process (own venv),
   exposing a JSON-RPC/HTTP surface the panel proxies. *Pro:* real process boundary,
   per-plugin dependency isolation, crash containment. *Con:* IPC latency, a stable
   RPC contract to maintain, lifecycle/health management, harder access to shared
   `db`/services (must go through a mediated API).

2. **Container-per-plugin.** Each plugin runs in its own container; the panel talks
   to it over the network. *Pro:* strongest isolation, resource limits (cgroups),
   reuses the Docker layer ServerKit already manages. *Con:* heaviest; startup cost;
   the panel already assumes single-worker in-memory agent state — orchestrating
   plugin containers adds real complexity.

3. **In-process capability confinement (Python).** RestrictedPython / import hooks /
   audit hooks to block undeclared imports and syscalls. *Pro:* no IPC. *Con:*
   CPython sandboxing is famously leaky; not a real boundary. Rejected as a security
   control (fine only as a lint/nudge).

## Recommendation

Keep the current posture. If escalation is triggered, prefer **(1) subprocess + RPC**
as the first step (smaller blast radius than containers, isolates deps), reserving
**(2) container-per-plugin** for genuinely hostile multi-tenant scenarios. Either way
the SDK is the seam: because plugins depend on `app.plugins_sdk` rather than host
internals, the SDK can become the RPC client without changing plugin code.

Out of scope for the current plan; revisit when the registry opens up.
