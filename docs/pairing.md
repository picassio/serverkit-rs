# Agent Pairing (Short-Code Flow)

ServerKit supports two ways to attach an agent to the panel:

1. **Pair (recommended)** — install the agent once, then claim it from the panel
   with a 6-character rotating code + your operator passphrase. RustDesk-style.
2. **Token install (legacy)** — generate a registration token in the panel,
   paste it into a curl/PowerShell one-liner. Still supported.

This document covers the pair flow.

## Why pairing?

The pre-shared registration token approach has several downsides:

- The token is a long-lived secret embedded in a copy-pasted shell command.
- If anyone observes the install command, they can pre-empt the registration.
- Re-pairing after a wipe means generating a fresh token, repeating the dance.

Pairing replaces this with:

- A **per-machine passphrase** the operator sets once on the agent host.
- A **rotating 6-character pair code** the agent displays (CLI banner / tray).
- A **public-key fingerprint** that's verifiable out-of-band.

The panel claims the agent only after both the code and passphrase match.

## Operator flow

### 1. On the target machine

After installing the agent (e.g. via the install script, package, or MSI), run:

```bash
serverkit-agent pair --server https://panel.example.com
```

You'll be prompted for a passphrase (4+ characters). Pick something memorable —
you only need to type it once on the panel side. The agent then:

- Generates an Ed25519 keypair (encrypted at rest with the machine key).
- Submits its public key + bcrypt'd passphrase to the panel.
- Prints a banner with the rotating 6-char pair code and the public-key fingerprint:

```
┌──────────────────────────────────────────────┐
│   ServerKit Agent — Pairing                  │
├──────────────────────────────────────────────┤
│   Pair code:   ABC-123                       │
│   Fingerprint: 1A2B3C4D5E6F7A8B               │
├──────────────────────────────────────────────┤
│   1. Open ServerKit panel → Add Server       │
│   2. Enter the pair code + your passphrase   │
│   3. Verify the fingerprint matches          │
└──────────────────────────────────────────────┘
Waiting for claim...
```

The pair code rotates every ~5 minutes until the agent is claimed.

### 2. In the panel

Go to **Servers → Add Server** and select **Pair existing agent**. Enter:

- The 6-character pair code
- The passphrase you set
- (Optional) A friendly name and group

Verify the fingerprint shown after lookup matches what the agent printed.
Click **Pair Agent**. The agent receives credentials over its existing
long-poll, saves them, and prints `✓ Pairing successful!`.

### 3. Start the agent

```bash
sudo systemctl start serverkit-agent   # Linux
# or use Services.msc on Windows
```

The agent connects to the panel and the server flips from "Pending" to "Online".

## Headless / unattended pairing

For Docker, Kubernetes, or unattended provisioning:

```bash
SERVERKIT_AGENT_PASSPHRASE='hunter2' \
  serverkit-agent pair --server https://panel.example.com --headless
```

You still need an operator to claim the code in the panel — the headless flag
just suppresses interactive prompts.

## Local dev / ngrok

If you're testing the panel locally, expose it via ngrok and set:

```bash
export SERVERKIT_PUBLIC_URL=https://abcd1234.ngrok.app
```

Use that URL as the `--server` flag. For self-signed TLS in dev,
`SERVERKIT_INSECURE_TLS=true` skips cert verification on the agent side
(don't use this in production).

## Security notes

- Pair codes are 6 characters from a 31-symbol alphabet
  (Crockford-ish base32 minus 0/O/1/I/L). Codes rotate ~every 5 minutes.
- Passphrases are bcrypt'd on the panel before storage.
- Failed claim attempts trigger per-enrollment exponential backoff
  (60s → 5 min cap) plus per-IP rate limits (5 claims / 10 minutes).
- All claim/lookup events are written to the audit log.
- Enrollments expire 24h after creation; un-claimed records are pruned hourly.
- The agent's keypair is encrypted at rest with a **host-stable** key derived
  from `/etc/machine-id` + hostname (Linux) or hostname + COMPUTERNAME (Windows).
  The key is intentionally independent of the logged-in user so the Windows
  service (running as `LocalSystem`) can decrypt credentials written during
  user-context pairing — mixing in the Windows username broke this and was
  removed in agent 1.6.14 (a legacy USERNAME-based key is retained only for
  one-time migration). Treat the stored key file as a host-equivalent secret;
  see [../SECURITY.md](../SECURITY.md) for the full agent trust model.
- Compare the displayed fingerprint side-by-side before claiming. Mismatch
  means the wrong agent — abort and re-run `pair`.

> **Treat the stored credential/key file as a host-equivalent secret.** Combined
> with the agent's remote-execution capability, recovering it is equivalent to
> controlling the host. See [../SECURITY.md](../SECURITY.md) for the full agent
> trust model.

## API reference

| Endpoint | Purpose | Auth |
| --- | --- | --- |
| `POST /api/v1/pairing/enroll` | Agent submits pubkey + passphrase | none |
| `POST /api/v1/pairing/code/refresh` | Agent rotates its pair code | enrollment headers |
| `POST /api/v1/pairing/code/freeze` | Agent freezes/unfreezes rotation | enrollment headers |
| `GET  /api/v1/pairing/poll` | Agent long-polls for claim (≤25s) | enrollment headers |
| `POST /api/v1/pairing/lookup` | Panel resolves code → fingerprint | JWT |
| `POST /api/v1/pairing/claim` | Panel claims an enrolled agent | JWT |

Headers used by the agent:
- `X-Enrollment-Id`: opaque enrollment identifier
- `X-Enrollment-Secret`: long-lived secret returned at enroll time

## Comparison with token install

| | Pair | Token install |
| --- | --- | --- |
| Out-of-band step | passphrase typed into panel | token pasted into shell |
| Compromise window | 5 min (rotating code) | 24h (token TTL) |
| Re-pair after wipe | re-run `pair` | regenerate token |
| Headless friendly | yes (`--headless` + env) | yes (curl one-liner) |
| Recommended? | ✅ | only for legacy/scripted flows |
