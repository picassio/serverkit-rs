# Changelog

All notable changes to ServerKit are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Scope:** This changelog tracks the **control panel** (Flask backend + React
> frontend). The cross-platform **agent** ships on its own cadence and is tagged
> separately (`agent-vX.Y.Z`) — see [`agent/README.md`](agent/README.md) for its
> install and release notes.
>
> Commit-level history lives in `git log`; this file curates the user-facing
> changes by theme.

## [Unreleased]

The `dev` branch is well ahead of the last `main` release. The headline work
awaiting a stable release:

### Removed

- **Legacy marketplace catalog** — the DB-seeded `Extension`/`ExtensionInstall`
  catalog (a third, always-empty lane in Marketplace Browse) was retired. Browse
  now has exactly the real sources: bundled built-ins, the remote registry, and
  installed-plugin state. The orphaned `extensions`/`extension_installs` tables
  are dropped by migration 046 (they never held real data — nothing seeded them).

### Added

- **Extensions platform (Phase 7 — settings slot + manifest linting)** — extensions
  can now contribute sections to the Settings page (a `settings.section` widget
  slot rendered below the active tab), and `plugin.json` manifests are shape-checked
  at install time: malformed entry points, socket/model references, jobs, schedules,
  or contribution entries now fail the install with a message naming each problem
  instead of being silently dropped at runtime. Authors get the same rules locally
  via `node scripts/new-extension.mjs --validate <path>`.
- **Extensions platform (Phase 7 — scheduled update checks)** — the panel now
  checks the extension registry for updates once a day (a regular scheduled job,
  visible under Jobs) and notifies admins through the Notifications Bus when new
  versions are available — once per release set, not once per day. The
  Marketplace "Update available" badge remains the always-current surface.
- **Extensions platform (Phase 7 — per-extension configuration)** — an extension
  that declares a `config_schema` in its manifest now gets a real **Configure**
  form on the Marketplace Installed tab (text/number/boolean/enum/secret fields);
  values persist on the panel and the extension's backend reads them with the new
  `plugins_sdk.config(slug)` accessor. Config values may hold secrets, so they are
  served only by an admin-gated endpoint and never appear in plugin listings.
- **Extensions platform (Phase 7 — installed extensions survive panel updates)** —
  previously a panel update deployed a fresh source tree preserving only `.env`
  and the database, silently wiping any URL/registry/upload-installed extension's
  files (the install row then flipped to `error` on the next boot). Two-layer fix:
  the updater now carries user-installed plugin directories forward into the new
  tree (before the frontend build, so their UI recompiles), and the backend gained
  a boot-time repair pass that restores builtin installs from `builtin-extensions/`
  and re-downloads URL installs from their recorded source — upload-only installs
  it can't restore get a clear "re-upload" error instead of a cryptic import failure.
- **Extensions platform (Phase 7 — extensions can now contribute tabs to core
  tab groups)** — a new `tabs` contribution kind lets an installed extension add
  a real tab to a core-owned tab group (Files, Servers, Observability): the tab
  joins the shared top-bar strip, its routes render inside the group's layout so
  the chrome stays, and the group's sidebar item stays lit on the extension's
  routes. Four pages moved out of core onto it: **FTP Server → `serverkit-ftp`**
  (Files group), **Cloud Provisioning → `serverkit-cloud-provision`** and
  **Remote Access → `serverkit-remote-access`** (Servers group, keeping their
  original tab positions; the per-server Remote Access tab on the server detail
  page stays core), and **Status Pages → `serverkit-status`** (Observability
  group; the public `/status/<slug>` page stays core). Each tab + page + palette
  entry disappears together when its extension is uninstalled. Existing panels
  auto-install all four once on upgrade so nothing disappears; fresh installs
  find them in the Marketplace.
- **Extensions platform (Phase 5 — Cloudflare zone-ops is now a bundled extension)** —
  the Cloudflare per-zone control panel (zone settings, cache purge, WAF, Workers,
  Tunnels, and R2/KV/D1 storage, reached from the "Open in Cloudflare" button on a
  Cloudflare-managed domain) moved out of core into the **`serverkit-cloudflare-ops`**
  extension. It ships installed by default (a flagship — zero nav footprint, and the
  core Domains button depends on the route) and is uninstallable. Crucially, **DNS
  records and the Cloudflare connection stay core** (they back `/domains`): the
  extension borrows the single core Cloudflare API client rather than vendoring its
  own, so there's exactly one client and no credential duplication. API paths are
  unchanged (`/api/v1/cloudflare`, D9), and the `CloudflareWorker`/`CloudflareTunnel`
  models stay core (they key off the shared DNS-provider connection).
- **Extensions platform (Phase 5 — WordPress is now a bundled extension)** — the
  entire WordPress backend (site provisioning, plugin library, environments/
  pipelines, updates, security, vulnerability scanning, analytics & reports, and
  the `/api/v1/wordpress` API family) has moved out of core into the
  **`serverkit-wordpress`** extension. Because WordPress is a flagship, it ships
  **installed by default on every panel** and can be uninstalled to slim the core —
  it never becomes a Marketplace hunt (decision D4). API paths are unchanged
  (`/api/v1/wordpress`, `/projects`, and the `/pipelines` alias all survive, D9),
  and every WordPress model stays core so backups, Fail2ban, status pages, and
  environment activity keep their foreign keys. The old WordPress "module toggle"
  is retired — the extension's install/enable state is the gate. Core code reaches
  the extension's services through an importlib bridge, so a panel with WordPress
  uninstalled no longer loads any of the ~6k lines of WordPress service code. The
  **WordPress UI is contributed by the extension too** — a single `wordpress/*`
  route self-renders the whole WordPress sub-router (site list + plugin library +
  pipelines tab group, plus the full-bleed site/pipeline detail pages), and the
  sidebar item, command-palette entries, and page titles all come from the
  extension manifest. Uninstalling WordPress now cleanly removes its nav, routes,
  and API in one go.
- **Extensions platform (Phase 4 — Email is now an extension)** — the mail-server
  stack (Postfix/Dovecot, domains, mailboxes, DKIM/SpamAssassin, Roundcube webmail,
  and the `/api/v1/email` API) has moved out of core into the bundled
  **`serverkit-email`** extension. Panels that never run mail no longer load any of
  it — a real "smaller core" win. Existing panels that actually used mail
  auto-install the extension once on upgrade (detected by existing mail domains/
  accounts); everyone else finds it in the Marketplace. Outbound notification SMTP
  is unaffected — it never depended on the mail server. (The Email "module toggle"
  is retired in favor of installing/disabling the extension.)
- **Extensions platform (Phase 3 — platform primitives)** — the machinery that
  makes extensions first-class and safe. Extensions can now own **data models**
  (manifest `models` → `ext_<slug>_*` tables, created on install, dropped on
  purge), **background jobs & schedules** (wired into the Jobs system, and paused
  automatically when the extension is disabled), and a **real-time Socket.IO
  namespace** (`/ext/<slug>`, status-guarded). Declared **permissions** are now a
  consent step and enforced by an SDK capability gate (`require_permission`).
  **Panel-version compatibility** (`min_panel_version`/`max_panel_version`) is
  enforced at install and update. Uninstall offers **keep-data vs purge**. New
  generic **contribution slots** (`dashboard.top`, `service.detail.tab`,
  `domain.drawer.panel`) let extensions enrich core surfaces, not just add pages.
  The frontend-delivery decision is recorded in
  [`docs/adr/0001-extension-frontend-delivery.md`](docs/adr/0001-extension-frontend-delivery.md).
- **Extensions platform (Phase 2 — remote registry & updates)** — the Marketplace
  Browse tab can now show extensions from a curated remote **registry** (a single
  `index.json`), merged in and labeled "Registry", with no per-panel seeding. The
  fetch is offline-tolerant (last-good cache → a bundled fallback index) and
  cached. Installing from the registry is **checksum-verified** — the downloaded
  zip's sha256 must match the index before extraction, or the install hard-fails.
  Panel-version gates (`min_panel_version`/`max_panel_version`) block installs a
  panel is too old to run. Installed extensions listed in the registry now get an
  "Update available" badge and a one-click **Update**. Format + publishing guide in
  [`docs/EXTENSIONS_REGISTRY.md`](docs/EXTENSIONS_REGISTRY.md).
- **Extensions platform (Phase 1 — seed the marketplace)** — the Marketplace is
  now genuinely populated. **GPU Monitor** and **Workflow Builder** became bundled
  builtin extensions (`serverkit-gpu`, `serverkit-workflows`) — same route, but
  their nav/route/title/command-palette entries now come from the extension
  manifest. An upgraded panel auto-installs a converted builtin once so nothing
  disappears; fresh installs simply see it in the Marketplace. New **Module
  toggles** (Settings → Modules) let an admin hide the Email and WordPress
  verticals — nav, routes, and the module's API (`/api/v1/email`,
  `/api/v1/wordpress`) all switch off — for a smaller panel without uninstalling
  anything. The Marketplace gained a "by ServerKit" first-party badge, real
  category chips, and an extension detail view with icon + screenshots.
- **Extensions platform (Phase 0 — hygiene)** — groundwork for the small-core +
  marketplace direction. A single **extension author guide**
  ([`docs/EXTENSIONS.md`](docs/EXTENSIONS.md)) documents the manifest schema,
  contribution envelope, lifecycle hooks, backend SDK, install sources, and the
  production frontend-delivery constraint. Builtin-extension frontends are now
  mechanically kept in sync with their source
  (`scripts/sync-builtin-frontends.mjs` + an `Extensions CI` drift gate) instead
  of hand-duplicated. The Marketplace labels bundled entries honestly ("Built-in"
  rather than "Local mapping/Entries"). First automated coverage for the plugin
  install pipeline (builtin install, contributions envelope, disable→503 guard,
  reinstall metadata refresh, zip-slip rejection).

- **Managed databases** — the databases ServerKit provisions are now tracked as
  first-class resources (beside the existing live explorer): durable rows for
  backups and connection strings. A managed database backs a `BackupPolicy` by a
  real foreign key (not an untethered descriptor), one-click "Protect" creates
  that policy, and a real connection URI can be revealed/copied (audited, secret
  Fernet-encrypted at rest). Adopt an existing database to start tracking it. API
  under `/api/v1/databases/managed`. Not a DBaaS — no pooling/replicas/scaling.
- **Per-app managed volumes** — first-class, tracked persistent storage for a
  service. Attach a named Docker volume at a chosen container path under
  Settings → Storage; it survives redeploys and is visible with live
  present/size state, instead of a fragile relative bind mount
  (`./mysql-data:/var/lib/mysql`). Detaching keeps the data by default; wiping is
  blocked while the app runs. API under `/api/v1/apps/<id>/volumes`.
- **Private container registries** — store credentials once under Settings →
  Connections (GHCR, Docker Hub, GitLab, ECR, or any generic registry) and
  ServerKit runs `docker login` before pulling a private image, then logs out.
  Secrets are Fernet-encrypted at rest and piped via stdin (never on argv);
  attach a registry to a service under Container Ops. Anonymous pulls are
  unchanged. API under `/api/v1/connections/registries`.
- **Container status aggregator** — collapses an app's per-container Docker
  states into one deterministic status (`running:healthy` … `degraded` …
  `unknown`) at `/api/v1/status/app/<id>` and `/api/v1/status/apps`, with
  change-only pushes over the `container_status` Socket.IO channel.
- **API token scopes** — fine-grained, additive scopes for API keys (enforced
  only for `X-API-Key` requests; JWT/session callers stay RBAC-governed), a
  `require_scope` decorator, and a scope catalog at `/api/v1/api-keys/scopes`.
- **Server onboarding state machine** — a linear lifecycle (validating →
  installing prerequisites → installing Docker → pairing agent → ready/failed)
  driven on the job bus, with start/retry/status at
  `/api/v1/servers/<id>/onboarding/*` and an ordered progress log.
- **Declarative template catalog** — a documented catalog schema
  (`/api/v1/templates/catalog/schema`) with auto-resolved `${SERVICE_*}` magic
  variables (password/user/FQDN/URL/base64) so templates never hardcode generated
  secrets or hosts. See [docs/TEMPLATE_CATALOG_SCHEMA.md](docs/TEMPLATE_CATALOG_SCHEMA.md).
- **Build packs** — zero-Dockerfile detection that inspects a repo and generates
  a Dockerfile + compose from a build plan (`/api/v1/buildpacks/detect`,
  `/generate`), persisted on the application row; defers to an author-provided
  Dockerfile when present.
- **Deployment config snapshots** — immutable, secret-masked config snapshots
  captured before each deploy, with diff and one-click restore + redeploy at
  `/api/v1/apps/<id>/snapshots[/<id>/diff|/restore]`.
- **Projects & Environments** — a Workspace → Project → Environment → Apps
  hierarchy (`/api/v1/projects`, `/api/v1/environments`) with workspace-scoped
  access and resource counts.
- **Shared resources** — polymorphic tags and attachable shared variable groups
  with a merged "resolved" view and masked secrets (`/api/v1/shared/...`).
- **PR preview environments** — ephemeral previews driven by a pull-request
  webhook (`/api/v1/webhooks/pull-request/<token>`) that open, redeploy, and tear
  down per PR, managed at `/api/v1/apps/<id>/previews`.
- **Per-server managed proxy stack** — opt-in Dockerized Traefik or Caddy per
  server with a compose preview before switching, host nginx remaining the
  default (`/api/v1/servers/<id>/proxy*`).
- **Multi-platform agent & fleet management** — native Go agent for Linux,
  Windows, and macOS with HMAC-SHA256 auth and WebSocket + HTTP-poll transports,
  plus a fleet dashboard (inventory, connection status, approval queue,
  discovery, rollouts, and command queue).
- **Native Windows agent** — Windows service, desktop setup wizard (WebView2),
  system tray, and MSI installer; also `.deb`/`.rpm` packages and ARM64 builds.
- **Agent pairing** — short-code and passphrase pairing flows with keypair
  enrollment, the `sk1` connection-string format, and automatic fallback to
  polling when WebSocket connections flap.
- **Remote operations over the agent** — files, packages, services, cron, sudo,
  Docker, Cloudflare tunnels, and streamed job progress on connected servers.
- **Plugin / extension system** — plugin SDK, contribution points, capabilities
  and permissions, marketplace UI, built-in extensions, and a GUI plugin
  (`serverkit-gui`).
- **Status pages** — public status pages with HTTP/TCP/DNS/Ping checks,
  component monitoring, and incident management.
- **Cloud provisioning** — provision servers on DigitalOcean, Hetzner, Vultr,
  and Linode with cost tracking.
- **Git-based services** — GitHub source connections, repository picker,
  manifest detection, and "New Service from repo" (Git extension canonical at
  `/git`).
- **RHEL-family support** — the installer now covers Rocky, AlmaLinux, RHEL, and
  CentOS in addition to Ubuntu/Debian/Fedora.
- **Per-app Web Application Firewall** — ModSecurity v3 + OWASP Core Rule Set
  with detect/block modes, paranoia tuning, a disabled-rule editor, one-click
  apply (nginx include injection), and parsed audit-log events.
- **Container lifecycle controls** — image-update detection with one-click
  apply, idle container auto-sleep, and CPU-driven horizontal auto-scaling, with
  cron-drivable sweeps for the sleep/scale policies.
- **GPU monitoring** — NVIDIA utilization, memory, temperature, power, and
  per-process / per-container usage.
- **Dynamic DNS** — token-authenticated A/AAAA updates synced through a
  connected DNS provider (e.g. Cloudflare).
- **Secrets manager & inbound webhook gateway** — encrypted secret storage and
  inbound webhook endpoints for triggering automation.
- **Passkeys / WebAuthn** — passwordless and second-factor authentication with
  hardware keys, Touch ID, and Windows Hello.
- **Remote service tunnels** — expose a private or NAT'd service through an edge
  server over an agent-managed, NAT-traversing WireGuard tunnel, reusing nginx,
  DNS, and certificates.
- **Connections hub** — a single place to link external accounts (source code,
  cloud, DNS, domain registrars with expiry tracking, SMTP relays, and S3/B2
  storage), with credentials encrypted at rest.
- **WordPress publishing** — publish managed sites at a real subdomain, swap a
  site's URL safely with preview, and attach a custom domain with automatic DNS.
- **Guided installer / updater** — health-checked install and update flow with
  automatic rollback.

### Security

- **Container CVE scanning & SBOM** — per-image vulnerability scans with grype
  and software bill-of-materials generation with syft.
- **Optional, hardened TLS** — best-effort HTTPS that never blocks an install
  (falls back to HTTP), a server-wide TLS 1.2+/AEAD-cipher floor applied at
  install and update, Cloudflare-aware nginx configs, automatic CAA records on
  certificate issuance, and HSTS gated on the operator's recorded SSL choice so
  HTTPS stays optional.
- **Encryption at rest** — system-setting secrets and DNS/cloud provider
  credentials sealed with Fernet; optional client-side backup encryption.

### Changed

- Overhauled the Docker UI (bulk container stats, compose listing) and migrated
  the frontend design system to SCSS `.ui-*` components.
- Unified the local dev launcher (`dev.sh` / `dev.ps1`).
- Agent capabilities and system info are cached to the database, surfaced in the
  System Status card, and re-sent on a periodic cadence.

### Fixed

- Resolved systemic silent failures: empty logs, dead WebSocket connections, and
  locked-out agents; stale "online" status is now auto-corrected.
- Hardened the installer: Docker install on Fedora/RHEL, SELinux + nginx
  reverse-proxy configuration, and low-RAM swap setup.
- Stopped dropping capability/sysinfo payloads on transient `/poll` failures.
- **Extension pages no longer ghost-render on every route** — the plugin
  loader's legacy auto-render (any plugin index default export renders
  globally) only excluded plugins declaring a *widget* contribution, so the
  WordPress and Cloudflare-ops builtins mounted their whole pages on every
  navigation: pages stacked into one view, the Cloudflare page fetched
  `zones/undefined`, and the WordPress sub-router swallowed the current URL as
  a site id. Legacy auto-render now skips any plugin declaring any
  contribution, and the surplus default exports were removed.
- **Live updates (WebSocket) actually work now** — the deployed gunicorn
  worker class (gevent-websocket) double-answered the WebSocket handshake
  against the app's `threading` async mode; browsers reported "Invalid frame
  header" and every panel silently fell back to polling. The service unit and
  Docker image now run a plain threaded worker (still a single process, which
  the agent gateway requires) with `simple-websocket` serving the socket.
- Settings → About now reports the real panel version on custom-directory
  installs and in Docker: version resolution honors the install location
  (`SERVERKIT_INSTALL_DIR`, rendered into the service unit from the installer's
  `SERVERKIT_DIR`), prefers the running tree over a stale `/opt/serverkit`, and
  the Docker image now ships the `VERSION` file (containers previously showed
  the `1.0.0` fallback). The File Manager's "Stack" quick link follows the same
  resolved install directory instead of assuming `/opt/serverkit`, and when
  browsing a remote agent the quick-access rail now matches that box: Linux
  agents get their agent config dir alongside the generic paths, Windows agents
  get `ProgramData\ServerKit\Agent` + `C:\Users` instead of Unix paths that
  don't exist there. Agents newer than v1.0.4 self-report their real install
  and config directories in `system_info` (stored on the server record,
  migration 047), and the rail prefers those over the installer conventions.
- **Scripts reliability (round 2)** — swept the whole install/update/uninstall/CLI
  shell surface for the "benign non-zero under `set -e`/`pipefail`" failure class
  behind the July 2 update outage:
  - **Data loss closed:** re-running `install.sh` over a live install no longer
    destroys `.env` (secret keys) and the SQLite database — it now detects the
    existing install correctly and carries live state across the re-deploy.
  - The default **no-domain curl-pipe install** works again (aborted at the nginx
    phase since v1.6.25); blank Enter at the interactive domain prompt no longer
    aborts; `--release` updates can complete (progress output was corrupting the
    captured tarball path); updates on boxes with no prior backups no longer
    report failure after succeeding.
  - Rollback can no longer run twice or abort mid-flight; uninstall never aborts
    mid-teardown and works on remnants-only boxes; `serverkit start/restart/logs/
    add-site` degrade gracefully on partial installs, non-systemd, and RHEL-family
    nginx layouts; `serverkit doctor` exits non-zero when checks fail.
  - Agent enrollment: version discovery now pages past 30 releases (panel releases
    could push every agent tag off page 1), the panel injects its known agent
    version into the served installer, and the downloaded agent binary is
    checksum-verified. Package installs, swap setup, Docker bootstrap, and the
    Rocky/RHEL 9 OpenSSL/OpenSSH upgrade ordering are hardened across distro
    families (incl. Fedora 41+ dnf5 and busybox/Alpine).

### Testing & Infra

- Added a Vagrant + Hyper-V runner (Debian/Fedora/Rocky) and a Multipass-based
  end-to-end harness that runs on Windows.
- Shell-script test harness: a fresh-minimal-box loop now proves every
  observation/discovery function in `install.sh` and `update.sh` survives a
  zero-app box under strict mode (the July 2 outage class), backed by a shared
  failing-stub library and new `test_cli.sh`/`test_agent_install.sh` suites
  (171 assertions total across the five suites).
- Scripts CI now performs a **real install + update end-to-end** on every PR
  touching `scripts/**` (the updater self-updates from `main`, so this gates the
  fleet), plus nightly release-tarball install and update-from-latest-release
  jobs and an advisory full-severity shellcheck pass.

---

## Released

Current development version: **1.6.7**. Recent point releases (`1.4.x` → `1.6.7`)
delivered the agent fleet, plugin system, and installer hardening listed above.
Until tagged panel releases land, consult `git log` and the
[GitHub releases page](https://github.com/jhd3197/ServerKit/releases) for the
detailed history.
