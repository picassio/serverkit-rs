<div align="center">

# ServerKit-RS

**A self-hosted server control panel — Rust backend, React UI, built-in AI operator.**

Manage web apps, databases, Docker, nginx/PHP, and full **Magento** store lifecycles
from one dashboard. The in-panel assistant can *actually operate* the box: create stores,
run Magento actions, control containers, back up databases — with your own Claude/OpenAI
subscription, logged in from the browser.

[![CI](https://github.com/picassio/serverkit-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/picassio/serverkit-rs/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/picassio/serverkit-rs?sort=semver)](https://github.com/picassio/serverkit-rs/releases)

</div>

---

## What is this?

ServerKit-RS is a ground-up **Rust rewrite** of the [ServerKit](https://github.com/jhd3197/ServerKit)
backend (originally Python/Flask), keeping the polished React frontend and adding a
first-class Magento extension and an AI operator. Zero Python in the serving path — a single
`sk-server` binary plus a small Node sidecar for the AI SDK.

Design choice throughout: **data services (MariaDB, Redis, OpenSearch, RabbitMQ, …) run in
Docker**, while nginx + PHP-FPM run natively — the hybrid model used for real Magento dev/prod boxes.

## Highlights

- **🦀 Single Rust binary** — axum + tokio + socketioxide, SQLite via sqlx, wire-compatible with the existing React SPA.
- **🛒 Magento lifecycle** — one-click provisioning (Composer + data-plane containers + install + vhost + cron), quick actions (cache/reindex/upgrade/mode/maintenance), health, Varnish FPC, headless (shared/separate/split), managed frontends, per-store PHP + run-user, editable vhosts, DB backups.
- **🔒 Let's Encrypt, pure Rust** — HTTP-01 **and** DNS-01 (Cloudflare), with auto-renewal. No certbot.
- **🐳 Docker, terminal, files, logs, processes, services** — the usual panel surfaces, rendered by the unchanged UI.
- **📊 Monitoring & alerts** and a **marketplace** of 100+ app templates (`docker compose` under the hood).
- **🤖 AI operator** — the assistant streams over SSE and can call **native tools** to run real operations, gated by your RBAC. Powered by the [`pi`](https://github.com/mariozechner/pi-coding-agent) SDK in a bundled sidecar.
- **🔑 Bring-your-own subscription** — log in to **Claude Pro/Max**, ChatGPT, or Copilot from the browser (Settings → AI Assistant → Provider Login). Claude subscription billing works via the bundled `cc-patch`.
- **📦 Self-contained** — `install.sh` builds/deploys, generates secrets, installs a systemd service, and can bootstrap the first admin. The AI sidecar auto-starts and dies with the parent (no orphans).

## Architecture

```
                    ┌─────────────────────────────────────────────┐
   Browser ───────▶ │  sk-server (Rust: axum + socketioxide)       │
   (React SPA,      │   /api/v1/*   +  Socket.IO   +  static SPA    │
    served by       │                                              │
    sk-server)      │   sk-{auth,magento,web,db,docker,ops,files,  │
                    │       system,monitor,templates,acme,...}     │
                    └───────┬───────────────────────┬──────────────┘
                            │ spawns + supervises    │ manages
                            ▼                        ▼
            ┌───────────────────────────┐   nginx + PHP-FPM (native)
            │  ai-sidecar (Node, pi SDK)│   Docker data services
            │  cc-patch + sk_ tools     │   (MariaDB/Redis/OpenSearch/…)
            │  /auth/* login flow       │   Let's Encrypt (HTTP-01/DNS-01)
            └───────────────────────────┘
```

- **Backend** (`backend-rs/`): a Cargo workspace of focused crates (`sk-core`, `sk-auth`, `sk-magento`, `sk-web`, `sk-db`, `sk-docker`, `sk-acme`, `sk-monitor`, `sk-templates`, …) behind `sk-server`.
- **Frontend** (`frontend/`): the React SPA, built to static assets and served by `sk-server`.
- **AI sidecar** (`backend-rs/ai-sidecar/`): a Node service embedding the `pi` SDK — real `AgentSession`s, streaming, in-process tools, and the OAuth login flow. Auto-started by `sk-server`.

## Quick start

```bash
curl -fsSL https://raw.githubusercontent.com/picassio/serverkit-rs/main/install.sh | sudo bash
```

Or from a checkout / release tarball:

```bash
sudo ./install.sh
# non-interactive:
SK_BOOTSTRAP_ADMIN_EMAIL=you@example.com \
SK_BOOTSTRAP_ADMIN_PASSWORD='at-least-8-chars' sudo ./install.sh
```

The installer:
1. builds (or uses the prebuilt binary), installs to `/opt/serverkit-rs`,
2. generates `SECRET_KEY` / `JWT_SECRET_KEY` / `SERVERKIT_ENCRYPTION_KEY` into `/etc/serverkit/serverkit.env` (chmod 600),
3. installs + starts the `serverkit` systemd service,
4. bootstraps the first admin (interactively or via `SK_BOOTSTRAP_ADMIN_*`).

Then open `http://<host>:5000`. If you didn't bootstrap, the **first account you register becomes admin**.

## Development

```bash
# backend
cd backend-rs
cargo test
DATABASE_URL="sqlite:///tmp/serverkit.db" PORT=5055 \
  SK_FRONTEND_DIST=../frontend/dist \
  SK_DATA_DIR=/tmp/sk-data \
  SK_SIDECAR_DIR=$PWD/ai-sidecar \
  cargo run --bin sk-server

# frontend (dev server or build)
cd frontend
npm ci
npm run build        # produces frontend/dist that sk-server serves
```

Requires Rust ≥ 1.94 (socketioxide 0.18) and Node ≥ 18.

## Configuration

Set via the environment (see [`serverkit.env.example`](serverkit.env.example)):

| Variable | Purpose |
|---|---|
| `PORT` | HTTP port (default 5000) |
| `DATABASE_URL` | `sqlite://…` path |
| `SK_DATA_DIR` | runtime data (encryption key, monitor state, cron) |
| `SK_FRONTEND_DIST` | built SPA to serve |
| `SK_TEMPLATES_DIR` | marketplace templates |
| `SK_SIDECAR_DIR` | AI sidecar directory (enables auto-start) |
| `SECRET_KEY`, `JWT_SECRET_KEY` | app + JWT secrets (**change in production**) |
| `SERVERKIT_ENCRYPTION_KEY` | Fernet key for store credentials at rest |
| `SK_CF_API_TOKEN` | Cloudflare token for Let's Encrypt DNS-01 |
| `SK_BOOTSTRAP_ADMIN_*` | non-interactive first-admin bootstrap |
| `SK_SIDECAR_URL` / `SK_SIDECAR_TOKEN` | use an externally-managed sidecar instead of auto-start |
| `SK_SIDECAR_AUTOSTART=0` | disable AI sidecar auto-start |

## The AI operator

- **Assistant mode** enables native `sk_*` tools; **Simple mode** is chat-only. Tool calls run against the ServerKit API *as the requesting user*, so RBAC, validation, and protected-resource guards all apply.
- Tools cover: list/create/delete Magento stores, run Magento actions, back up DBs, control containers, create nginx sites, install templates, add cron jobs, read metrics/monitoring, and more.
- **Provider login** (Settings → AI Assistant → Provider Login): start a login, open the URL, paste back the redirect/code — credentials are stored server-side. Claude Pro/Max subscription billing is enabled by the bundled `cc-patch`.
- The sidecar is fully self-contained (vendors the `pi` SDK) and is auto-started/supervised by `sk-server`.

> **Security note:** in Assistant mode the agent also has pi's built-in `bash`/`read`/`write` tools (runs as the server user, bypassing panel RBAC). Restrict or disable for multi-tenant use.

## CI / Releases

- **CI** ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)): builds + tests the Rust backend, builds the frontend, and smoke-checks the sidecar on every push/PR.
- **Release** ([`.github/workflows/release.yml`](.github/workflows/release.yml)): on a `v*` tag, builds a Linux x86-64 tarball (`sk-server` + `frontend/dist` + `ai-sidecar` + `templates` + `install.sh`) with a SHA-256, and publishes a GitHub Release.

```bash
git tag v0.1.0 && git push origin v0.1.0     # triggers a release build
```

## Security

- Secrets live only in `/etc/serverkit/serverkit.env` (chmod 600) and are gitignored; the repo ships only `serverkit.env.example`.
- Store credentials are encrypted at rest (Fernet, `SERVERKIT_ENCRYPTION_KEY`).
- Passwords use werkzeug-compatible scrypt hashing; JWTs are flask-jwt-extended-compatible.
- Change the default `*-change-in-production` secrets before exposing the panel.

## Credits & license

A fork/rewrite of [jhd3197/ServerKit](https://github.com/jhd3197/ServerKit). AI powered by
[pi-coding-agent](https://github.com/mariozechner/pi-coding-agent). See [LICENSE](LICENSE).
