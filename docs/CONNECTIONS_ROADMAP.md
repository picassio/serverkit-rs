# ServerKit Connections Roadmap

*Turning Settings → Connections from a 3-provider tab into the **single front door** for every external account ServerKit already runs on — source hosts, cloud servers, DNS, registrars, mail and storage — and the one place to answer "what do I own, what's it connected to, and when does it lapse."*

---

## The thesis: surface and unify, not build from zero

The Connections hub looks like a thin GitHub-plus-DNS tab. It isn't. ServerKit **already** talks to most of these providers — but the credentials and the capability are scattered across **four storage locations and three different pages**, so none of it reads as "your connected accounts in one place."

| Account type | Backend today | Where creds live | Powers | In the hub? |
|---|---|---|---|---|
| **GitHub** source | `source_connection_service.py` (OAuth) | Fernet-encrypted (`SourceConnection.access_token_encrypted`) | New Service from a repo | ✅ live |
| **Cloudflare / Route 53** DNS | `dns_provider_service.py` (dispatcher) | ⚠️ **plaintext** (`DNSProviderConfig`) | Wildcard SSL, email DNS, custom-domain attach | ✅ live |
| **Cloud servers** (DigitalOcean / Hetzner / Vultr / Linode) | `cloud_provisioning_service.py` + `models/cloud_server.py` + `api/cloud_provisioning.py` | (in cloud-provisioning config) | **Servers** page | ❌ not surfaced |
| **S3 / Backblaze B2** object storage | `storage_provider_service.py` (boto3) | ⚠️ masked in `storage.json`, **not** Fernet | **Backups** page | ❌ not surfaced |
| **Domain registrar / expiry** | — **none** — | — | **Domains** page | ❌ doesn't exist |

Three consequences fall out of that table:

1. **Two of the user's three asks are mostly *wiring*, not building.** "DigitalOcean to manage servers" → the provisioning backend exists; it's just never been surfaced in the hub. "S3 in the Files app" → the S3 backend exists (it runs backups); it's never been pointed at the file browser.
2. **One ask is genuinely net-new: "when do my domains expire?"** `models/domain.py` only tracks `ssl_expires_at` (certificate expiry). There is no registrar, no registration/expiry date, no WHOIS anywhere in the models. That's the headline new feature, and it's the one that delivers the "all this info in one place" payoff.
3. **There's a real security gap to close on the way through.** `utils/crypto.py` (`encrypt_secret`/`decrypt_secret`, Fernet) is already used by source connections, AI, the agent fleet and pairing — but DNS and storage secrets sit in plaintext / lightly-masked. Every new provider should encrypt at rest, and the existing two should be migrated.

**So the fast path to "one place for everything" is surfacing what exists, completing the catalog's own "coming soon" tiles, then building the single genuinely-missing system (registrar + expiry).**

---

## ✅ Update (2026-06-15): roadmap complete — #1–#18 all shipped

Built in one session and verified — `npm run build` (frontend, clean), `eslint` (clean), `py_compile` + venv import (backend, clean). Runtime click-through with real provider credentials is still the operator's to do.

- **Phase 1 — surface what exists:** #2 Infrastructure category (DigitalOcean / Hetzner / Vultr / Linode → existing `/cloud`), #3 S3 + Backblaze B2 storage cards (→ existing `/backups/storage`), #4 cross-links on every connected card ("N servers — Servers →", "N domains — Domains →", bucket → Backups, "New service →").
- **Phase 2 — complete the tiles:** #5 GitLab source (full OAuth mirror of GitHub, 9 routes), #6 DigitalOcean DNS, #7 GoDaddy DNS (both as new dispatch branches in `dns_provider_service.py`), **#8 SMTP relay** — a real Postfix smarthost (`relayhost` + SASL `sasl_passwd`) with an OS-agnostic `smtplib` connection test; new `EmailRelayConfig` model (password Fernet-encrypted) + `/email/relay` routes + a presets-driven modal (Postmark / SES / Mailgun / SendGrid / custom). All four "coming soon" tiles are now live.
- **Phase 3 — domain ownership & expiry:** #9 `RegistrarConnection` model (**Fernet-encrypted** from the start), #10 `RegistrarService` (GoDaddy portfolio + expiry), #11 *not needed as written* — expiry is sourced **live** from the registrar, so no `domains`-table migration was required, #12 the `RegistrarPortfolio` panel on the Domains page ("N days left", colour-coded by urgency, Sync button), #13 registrar card shows "N domains · M expiring ≤30d". **Namecheap** is now live too (XML API; the connect form takes the API key, account username and a whitelisted server IP).
- **Phase 4 — S3 in the Files app:** #14 `/files/s3/*` endpoints (browse / read / write / delete / download-url / upload) on `StorageProviderService`, reusing the configured backup bucket; #15 an "S3 bucket" target in the File Manager — paths stay slash-rooted in the UI and map to keys server-side, virtual folders from prefixes, edit/upload/download/delete work, and folders/rename/permissions are disabled. Offered only when an S3/B2 destination is configured.
- **Phase 5 — encrypt secrets at rest:** #16 DNS-provider and storage secrets are now Fernet-encrypted (encrypt on write, `decrypt_secret_safe` at point-of-use with a plaintext fallback so legacy rows keep working); #17 an idempotent startup migration (`encrypt_legacy_secrets`) encrypts any pre-existing plaintext values in place. Added `decrypt_secret_safe` + `is_encrypted` to `crypto.py`.
- **#18 — credential unification (the optional cleanup):** the last plaintext store (`CloudProvider`, whose `api_key_encrypted` column held plaintext) now encrypts identically to the rest, so **all five stores** use the same `crypto` primitives + startup migration. Added a read-only `ConnectionRegistry` + `GET /api/v1/connections` that presents every connected account as one normalized, secret-free list (the "hub reads one list" single source of truth). A full write-side "one model replaces five" migration was deliberately **not** done — it would risk five working subsystems for no user-facing gain; the uniform encryption + read registry deliver #18's practical value.

New backend: `models/registrar_connection.py`, `services/registrar_service.py`, `api/registrars.py` (`/api/v1/registrars`, registered). New/changed frontend: expanded `providerCatalog.js` (6 categories, 16 tiles), rebuilt `ConnectProviderModal.jsx` (5 provider kinds), `ConnectionsHub.jsx`, `ProviderCard.jsx` cross-links, `ProviderBrands.jsx` icons, `components/domains/RegistrarPortfolio.jsx`, `services/api/connections.js`.

**Nothing open** — every task #1–#18 is done. Each external account type (source, infrastructure, DNS, registrar, email relay, storage) connects through the hub, secrets are encrypted at rest across all five stores, and `GET /api/v1/connections` lists them all in one normalized view.

**Post-roadmap polish:** (1) **domain-expiry notifications** — a daily scheduler fires through the notification channels when a registrar domain crosses 30/14/7/1 days or expires, de-duped via a small state file so each crossing alerts once; (2) **S3 image preview** — File Manager thumbnails and the preview drawer load bucket objects via short-lived presigned URLs (`ImageThumb` is S3-aware); (3) **encryption-health banner** — the hub now consumes `GET /api/v1/connections` and warns if any connection's credentials aren't encrypted at rest (invisible in the happy path).

**Encryption hardening + tests:** an audit of every secret read/write site caught one miss — `sites_https_service` handed *encrypted* DNS creds straight to the wildcard-cert issuer, so DNS-01 issuance would have received ciphertext and failed. Fixed via a public `DNSProviderService.decrypted_credentials()` (the canonical way to read DNS creds; nothing reads `.api_key` raw anymore), and locked down with `backend/tests/test_provider_secret_encryption.py` — 8 tests proving ciphertext-at-rest + plaintext-on-read across DNS/cloud/storage/registrar, the legacy-plaintext migration (idempotent), and the wildcard regression. 17/17 pass (with the existing `test_sites_https.py`).

---

## How to read this document

- Tasks are **globally numbered (#1 … #18)** so they're individually addressable — e.g. "do #9".
- Tasks are grouped into **Phases 0 → 5**, ordered so cheap, high-trust wiring lands before net-new systems. Phases are independently shippable and can be reordered.
- Each task lists **Today** (what exists, with paths), **Do** (the concrete change), **Reuse** (what to lean on), and **Done when** (acceptance).

**Effort legend** (single-developer estimate):

| Tag | Meaning |
|---|---|
| **S** | ≤ 2 hours — mostly wiring |
| **M** | ~ half a day to a day |
| **L** | 1–3 days |
| **XL** | multi-day epic; split before starting |

**Status legend:**

| Mark | Meaning |
|---|---|
| 🐛 | Security / correctness gap — worth fixing regardless of roadmap order |
| 🟡 | **Almost there** — the backend or model already exists; this is glue |
| ❌ | Net-new build |

---

## Target: the connections hub as one front door

Every external account in one categorized surface, each card showing **status + access scope**, and each linking to the page it powers:

`Source code` · `Infrastructure (servers)` · `DNS & domains` · `Registrars & ownership` · `Email & delivery` · `Storage & backups`

And the cross-links that make it "one place": a **DigitalOcean** card that says *"3 servers — Manage in Servers"*, an **S3** card that says *"Connected — Browse in Files · Used by Backups"*, a **GoDaddy** card that says *"12 domains · 2 expiring within 30 days — View in Domains."*

---

## Phase 0 — Declutter the hub (do first)

### #1 — Remove the redundant hub intro `[S]` 🟡 — ✅ Done
- **Today:** `ConnectionsHub.jsx` rendered a full intro block — accent icon + `<h2>Connections</h2>` + a description paragraph — directly under the sidebar's already-active **Connections** nav label. The heading duplicated the nav; sibling tabs (e.g. `NotificationsTab`) carry no such intro.
- **Do:** Delete the intro block so the first category leads the surface.
- **Landed:** Removed the `connections-hub__intro` block (icon + `<h2>` + `<p>`), the now-unused `connectedCount` computation and `Link2` import from `frontend/src/components/settings/connections/ConnectionsHub.jsx`, and the `&__intro` / `&__intro-icon` / `&__intro-text` rules from `frontend/src/styles/pages/_connections.scss`. `npm run eslint` on the component returns clean.
- **Follow-up (optional):** if the page still feels text-heavy, the per-category blurbs (`providerCatalog.js` `CONNECTION_CATEGORIES[].blurb`) are the next thing to trim — but they group rather than duplicate, so they earn their place.

---

## Phase 1 — Surface what already exists (glue, fast wins)

The highest value-per-hour in the whole roadmap: the backends are built and tested; they're just not in the hub.

### #2 — Add an **Infrastructure** category that surfaces cloud-server providers `[M]` 🟡
- **Today:** `cloud_provisioning_service.py`, `api/cloud_provisioning.py` and `models/cloud_server.py` already exist and are registered in `app/__init__.py` — ServerKit can talk to DigitalOcean / Hetzner / Vultr / Linode — but there is **no entry point in Connections**. (Verify the exact provider list + whether it supports *listing/importing existing* machines vs. only *creating* them, in `cloud_provisioning_service.py`, before wiring the card copy.)
- **Do:** Add a `{ key: 'infra', label: 'Infrastructure', blurb: 'Cloud accounts ServerKit can provision and manage servers in.' }` category to `providerCatalog.js`, plus DigitalOcean / Hetzner / Vultr / Linode provider entries with `kind: 'cloud'`. Give `ConnectProviderModal` a `cloud` branch: save the provider API token, then show a count + a "Manage in Servers" link.
- **Reuse:** `cloud_provisioning_service`, the existing `ProviderCard`/`ConnectProviderModal` shell, `ProviderBrands.jsx` (already imports `SiDigitalocean`).
- **Done when:** A user can paste a DigitalOcean token in Connections, see "Connected," and reach server provisioning from the card.

### #3 — Surface **S3 / Backblaze B2** storage in the hub `[S]` 🟡
- **Today:** `storage_provider_service.py` drives S3-compatible (AWS, Wasabi, MinIO, DO Spaces) + Backblaze B2 for offsite backups, configured only on the **Backups** page's "Storage" tab. The catalog already has a dimmed `s3` tile.
- **Do:** Flip the `s3` tile to live and add a Backblaze B2 tile, both reading/writing the **same** storage config the Backups tab uses (`PUT /backups/storage`). Card subtitle shows the bucket + "Used by Backups."
- **Reuse:** `storage_provider_service`, `api/backups.py` storage routes, `Backups.jsx` `PROVIDER_LABELS`.
- **Done when:** Storage credentials can be set from Connections *or* Backups interchangeably (one source of truth), test-connection works from the card.

### #4 — Cross-link every surfaced card to the page it powers `[S]` 🟡
- **Today:** Cards are self-contained; nothing tells you a connection feeds Servers / Files / Backups / Domains.
- **Do:** Add an optional `manageHref` + count to the card summary (e.g. "3 servers", "12 domains, 2 expiring"). Render a quiet link in `conn-card__meta`.
- **Done when:** Each connected card answers "what is this connected to?" at a glance — the core of the "one place" promise.

---

## Phase 2 — Complete the catalog's own "coming soon" tiles

These four are already drawn as dimmed tiles; the patterns to make them real are known.

### #5 — **GitLab** source connections `[M]` 🟡
- **Today:** `providerCatalog.js` already lists GitLab as `comingSoon`. GitHub's whole flow lives in `source_connection_service.py` (OAuth → Fernet-encrypted token → repo/branch/manifest listing) behind `api/source_connections.py`.
- **Do:** Replicate the GitHub methods for GitLab (different OAuth + API base + `PRIVATE-TOKEN`/`Bearer` header), accept `'gitlab'` in `models/source_connection.py`, add the mirrored routes, flip the tile to `kind: 'github'`-style with a GitLab OAuth-app admin form.
- **Reuse:** Everything in the GitHub path; the model's `(user_id, provider)` unique constraint already supports multiple providers per user. Cross-check `components/git/GitProviders.jsx` for any existing multi-provider scaffolding before duplicating.
- **Done when:** A user connects GitLab and picks repos on New Service exactly like GitHub.

### #6 — **DigitalOcean DNS** `[S]` 🟡
- **Today:** `dns_provider_service.py` dispatches on a `provider` string with `_cloudflare_*` / `_route53_*` methods; the API routes (`/email/dns-providers`) are already generic. Catalog has a dimmed `digitalocean` DNS tile.
- **Do:** Add `_digitalocean_*` branches (`https://api.digitalocean.com/v2/domains`, `Authorization: Bearer`) for list-zones / set-record / delete-record, and a form in `ConnectProviderModal`.
- **Reuse:** The dispatcher + `find_zone_for_domain` / `ensure_a_record` already power wildcard SSL and custom-domain attach — DO DNS inherits all of it for free.
- **Done when:** A DO-hosted domain can receive auto-created A/wildcard records like Cloudflare does.

### #7 — **GoDaddy DNS** `[M]` 🟡
- **Today:** Same dispatcher as #6. GoDaddy is also a **registrar** (see Phase 3) — start with its DNS role here.
- **Do:** Add `_godaddy_*` branches (`https://api.godaddy.com/v1/domains`, `Authorization: sso-key <key>:<secret>` → reuse the Route 53 two-field form shape), and a catalog entry.
- **Done when:** GoDaddy-hosted DNS records are managed through the same flow.

### #8 — **SMTP relay** for outbound mail `[M]` ❌ — ✅ Done
- **Today:** The `email` category exists with a dimmed `smtp` tile; the mail server has no outbound-relay connection object.
- **Do:** Add an `email`-kind connect form capturing host/port/user/pass/TLS (presets for Postmark / SES / Mailgun) and store encrypted; wire it as the relay for the mail server.
- **Done when:** Outbound mail can route through a configured third-party relay.
- **Landed:** `EmailRelayConfig` model (single-row, password Fernet-encrypted) + `EmailRelayService` (persist + `smtplib` connection test + apply) + `PostfixService.configure_relay`/`disable_relay` (sets `relayhost` + SASL `sasl_passwd`, `postfix reload`) + `/api/v1/email/relay` GET/PUT/POST-test/DELETE. Frontend: `smtp` tile → live `email` kind; modal `EmailBody` with provider presets (Postmark / SES / Mailgun / SendGrid / custom), STARTTLS + enable toggles, Test/Save/Disable. The Postfix apply is Linux-only and no-ops with a note off-Linux; the `smtplib` test validates credentials on any OS. Verified: `npm run build` + `eslint` (clean) and backend `py_compile` + venv import.

---

## Phase 3 — Domain ownership & expiry (the headline net-new: *"when do they expire?"*)

The one system that genuinely doesn't exist. This is what turns Domains from "DNS + certs" into "the portfolio of names you own, and when they lapse."

### #9 — `RegistrarConnection` model + encrypted credentials `[M]` ❌
- **Today:** No registrar concept anywhere.
- **Do:** New `models/registrar_connection.py` mirroring `SourceConnection`: `user_id`, `provider` (`godaddy`/`namecheap`/…), `provider_account_id`, `api_key_encrypted`, `api_secret_encrypted`, timestamps. Store secrets via `encrypt_secret` from the start (don't repeat the DNS plaintext mistake).
- **Reuse:** `utils/crypto.py`, the `SourceConnection` shape.
- **Done when:** A registrar account can be connected and its credentials are encrypted at rest.

### #10 — `RegistrarService` (GoDaddy, Namecheap) `[L]` ❌
- **Do:** Dispatcher service (`if provider == 'godaddy': _godaddy_list_domains()` …) exposing `list_domains()`, `check_expiry(domain)`, and later `update_nameservers()` / `renew_domain()`. GoDaddy: `GET /v1/domains`. Namecheap: `namecheap.domains.getList`.
- **Reuse:** The exact dispatcher shape from `dns_provider_service.py`.
- **Done when:** Given a connected registrar, the service returns each domain's registration + expiry date.

### #11 — Extend the `Domain` model with ownership fields `[S]` ❌
- **Today:** `models/domain.py` has only `ssl_expires_at` (cert), `ssl_auto_renew`, nginx/cert paths.
- **Do:** Add `registrar` (str), `registrar_connection_id` (FK), `registration_expires_at` (DateTime), `auto_renew_enabled` (bool), `nameservers` (JSON), `last_synced_at` (DateTime). Update `to_dict()`.
- **Done when:** A domain row can hold *who it's registered with and when it expires*, distinct from its cert.

### #12 — Expiry sync + the "expires in N days" UI `[M]` ❌
- **Do:** A periodic job (reuse the backup-schedule/cron mechanism) that calls `RegistrarService.check_expiry` for every domain with a `registrar_connection_id` and writes `registration_expires_at` + `last_synced_at`. On `pages/Domains.jsx`, show **expires in N days**, a warning pill under 30 days, auto-renew status, and a manual "Sync now."
- **Reuse:** Notification channels (#backups already proves the cron + notify path) for "domain X expires in 14 days" alerts.
- **Done when:** The Domains page is a true portfolio view: every name, its registrar, its expiry, color-coded by urgency.

### #13 — Registrar status on the Connections card `[S]` ❌
- **Do:** A "Registrars & ownership" category in the hub; GoDaddy/Namecheap cards show "12 domains · 2 expiring ≤30d — View in Domains" (the #4 cross-link, applied).
- **Done when:** The hub answers "is my domain ownership healthy?" without opening Domains.

---

## Phase 4 — S3 as a first-class Files target (*"S3 connects in the files app"*)

### #14 — `/files/s3/*` endpoints over the storage client `[M]` ❌ — ✅ Done
- **Today:** `file_service.py` / `api/files.py` are local-filesystem only (`ALLOWED_ROOTS`); no remote/pluggable backend. `storage_provider_service.py` already holds a working boto3 client.
- **Do:** Add `GET /files/s3/browse`, `/read`, `POST /files/s3/write`, `DELETE /files/s3/delete`, `GET /files/s3/info` that map `list_objects_v2` / `get` / `put` / `delete` into the same entry shape `browse` returns (virtual folders from `/`-suffixed prefixes).
- **Reuse:** The Phase 1 (#3) storage credentials — no new creds, no new config.
- **Done when:** The API can list and transfer bucket objects in the File Manager's data shape.
- **Landed:** `StorageProviderService.s3_browse / s3_read / s3_write / s3_upload / s3_delete / s3_presigned_get` + routes `/api/v1/files/s3/{browse,read,write,delete,download-url,upload}`. UI paths are slash-rooted and mapped to keys server-side; folders are virtual (CommonPrefixes); download hands back a short-lived presigned URL; delete removes a single object or a whole prefix. Reuses the configured backup bucket — no new creds.

### #15 — S3 bucket in the File Manager target picker `[M]` ❌ — ✅ Done
- **Do:** Add an "S3 bucket" option to `FileManager.jsx`'s target picker (next to Local + agents). When active, browse by prefix and **disable** ops S3 can't do (mkdir, rename, chmod).
- **Done when:** A user browses, uploads to, downloads from and deletes in their bucket from the Files app — the literal ask.
- **Landed:** `TargetPicker` gained a backward-compatible `extraOptions` prop; the File Manager shows an "S3 bucket" target when an S3/B2 backup destination is configured, routes browse/read/write/delete/download/upload through the existing `fileApi` adapter, disables folders/rename/permissions via the op guard, hides the local-only sidebar (quick-access/tree) on S3, and shows a context banner. Verified: `npm run build` + `eslint` (clean), backend `py_compile` + venv import.

---

## Phase 5 — Harden & unify credentials (security)

### #16 — Encrypt DNS + storage secrets at rest `[M]` 🐛 — ✅ Done
- **Today:** `DNSProviderConfig.api_key`/`api_secret` are plaintext; `storage.json` is only masked-on-read. `encrypt_secret`/`decrypt_secret` already exist and are used by source connections, AI, server and pairing.
- **Do:** Wrap reads/writes of DNS + storage + (new) registrar secrets in `encrypt_secret`/`decrypt_secret`.
- **Done when:** No third-party secret is persisted in plaintext.
- **Landed:** Added `decrypt_secret_safe` + `is_encrypted` to `crypto.py` (decrypt with plaintext fallback so encrypted and not-yet-migrated values coexist). DNS: encrypt in `add_provider`, decrypt at point-of-use via `_api_key`/`_api_secret` in the four header/client builders. Storage: encrypt in `save_config`, decrypt in `get_config` (so `_get_client` and the S3 browser get plaintext). Registrar + SMTP-relay secrets were already encrypted. Verified by an encrypt→decrypt round-trip + legacy-plaintext passthrough + `is_encrypted` detection test.

### #17 — Migrate existing plaintext secrets `[S]` 🐛 — ✅ Done
- **Do:** One-time migration that encrypts any already-stored DNS/storage secrets and records the migration in the migration-history surface.
- **Done when:** Existing installs are encrypted with zero user action.
- **Landed:** Idempotent `DNSProviderService.encrypt_legacy_secrets()` + `StorageProviderService.encrypt_legacy_secrets()` run on startup inside `create_app()`, encrypting any plaintext values in place (skipping already-encrypted ones via `is_encrypted`). Logs how many it touched; wrapped in try/except so a migration hiccup never blocks boot.

### #18 — (Optional) one credential abstraction `[L]` ❌ — ✅ Done (read facade + uniform encryption; write-model unification deliberately deferred)
- **Do:** Fold the four credential homes (SourceConnection, DNSProviderConfig, storage.json, cloud-provisioning config) behind a single `Connection` interface so every provider stores/scopes/encrypts identically and the hub reads one list.
- **Done when:** Adding a provider is one declarative entry + one service, with storage/scoping/encryption inherited.
- **Landed:** Delivered as the two valuable, low-risk halves rather than a five-subsystem rewrite. (1) **Uniform encryption** — fixed the last plaintext store: `CloudProvider.api_key_encrypted` (which held plaintext) is now Fernet-encrypted on write, decrypted at point-of-use in `_auth_headers`, with an idempotent `encrypt_legacy_secrets()` migration run at startup alongside the DNS/storage ones. All five stores now use the same `crypto` primitives. (2) **Unified read** — new `services/connection_registry.py` (`ConnectionRegistry.list_all`) + `api/connections.py` (`GET /api/v1/connections`, registered) return every connected account as one normalized, secret-free list; frontend `api.getAllConnections()`. Verified by booting `create_app('testing')` and running the registry live (returns a list; endpoint present in the URL map). **Not done (intentionally):** collapsing the five models into one and migrating their data — high risk, no user-facing benefit; the per-store write paths stay as-is.

---

## Future / nice-to-have

- **More source hosts:** Bitbucket, Gitea/Forgejo (replicate #5).
- **More registrars:** Cloudflare Registrar, Porkbun, Namecheap-resellers (extend #10).
- **More storage:** Cloudflare R2 / DigitalOcean Spaces / Wasabi presets (free — they're S3-compatible), then GCS / Azure Blob (new clients in `storage_provider_service`).
- **Import existing cloud servers** (not just provision new) — list a provider's droplets/instances and adopt them into the fleet, if `cloud_provisioning_service` doesn't already (verify).
- **Scoping policy:** decide per-category whether a connection is **per-user** (like GitHub) or **admin-wide** (like DNS) — registrars likely per-user, infra likely admin.

---

## Suggested order

1. **#1** ✅ (done) → **#2 #3 #4** — surface servers + S3 + cross-links. Biggest visible payoff for the least code; directly answers "DigitalOcean servers" and "S3."
2. **#9–#13** — domain registrar + expiry. The headline net-new "when do they expire," and the strongest "all in one place" moment.
3. **#14 #15** — S3 in Files.
4. **#5–#8** — finish the remaining catalog tiles as appetite allows.
5. **#16 #17** — encrypt-at-rest (pull earlier if any of this ships to production first).
