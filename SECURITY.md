# Security Policy

## Supported Versions

ServerKit is under active development. Security fixes are applied to the latest
release line and the `main` branch.

| Version | Supported |
|---------|-----------|
| 1.6.x   | ✅ |
| < 1.6   | ❌ (please upgrade) |

The **agent** is versioned independently (`agent-vX.Y.Z`); always run a recent
agent build, as several Windows service and credential-handling fixes landed in
the 1.6.x line.

## Reporting a Vulnerability

Please report security issues **privately** — do not open a public issue for
anything exploitable.

- Preferred: [open a GitHub Security Advisory](https://github.com/jhd3197/ServerKit/security/advisories/new)
- We aim to acknowledge reports within a few days and to provide a remediation
  timeline after triage.

Please include affected version(s), reproduction steps, and impact. Coordinated
disclosure is appreciated — give us a reasonable window to ship a fix before any
public write-up.

## Agent Trust Model

The multi-server agent is powerful by design — operators should understand its
trust boundaries:

- **`agent.key` is a host-equivalent secret.** Agent API credentials are stored
  AES-256-GCM encrypted under a key derived from host-stable identifiers
  (hostname + machine ID on Linux, hostname + computer name on Windows). Because
  that key is derived only from values available on the host itself, the
  encryption is at-rest tamper-resistance / off-host-exfil protection (e.g. a
  leaked backup) — **not** confidentiality against a local root/SYSTEM user, who
  can re-derive the key. Anyone who can read this file on the host can recover
  the credentials. The `0600` file permissions are the real access control;
  protect it like a root/SYSTEM secret.
- **Remote command execution is gated.** Arbitrary command execution
  (`system:exec`) and interactive PTY sessions are controlled by the agent's
  `Features.Exec` flag, which is **off by default**. Enable it only on servers
  where you intend the panel to run shell commands.
- **Transport & connection controls.** Agents authenticate to the panel with
  per-connection HMAC-SHA256 (with nonce/replay protection and a timestamp-skew
  check), and the panel enforces a per-server IP allowlist. Use `wss://`
  (TLS-terminated) in production.
- **`SERVERKIT_INSECURE_TLS=true` disables certificate verification** for all
  agent connections. It is intended for local development/testing only — never
  set it in production.

For a detailed internal audit of the panel, see
[SECURITY_AUDIT.md](SECURITY_AUDIT.md).

## Deployment Note

The agent gateway keeps all connected-agent state in-memory in a single process.
Run the panel with a **single** gunicorn worker process (threaded worker,
`-w 1 --threads N` — not the gevent-websocket worker class); multi-worker
deployments can misroute agent commands. See
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
