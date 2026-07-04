# ServerKit — Per-Site Edge & Horizontal Scaling Spec

*Splitting the #34 epic — and specifying the **keystone** it shares with #21, #15-TLS, #30-jail, and #8/#25.*

> **Status:** design only. No code shipped. This doc exists so #34 (and its
> prerequisites) can be executed in focused passes **on a real Linux/Docker
> host**, where the infra can actually be verified — which is exactly why it
> wasn't built blind from the Windows dev environment.

---

## 1. Why this doc

`docs/WORDPRESS_ROADMAP.md` defers **#34 (horizontal scaling)** as *"XL + blocked on
the same per-site reverse-proxy/upstream layer as #21."* That deferral is correct,
but it undersells how much already exists. A capability sweep of the nginx/SSL/storage
services shows the **load-balancing, rate-limiting, caching, per-vhost-log, wildcard-TLS,
and S3-upload primitives are already built** — they're just bound to the generic
`Application`/domain model, never to a `WordPressSite` running as a `localhost:PORT`
container.

So the real work is three layers, in order:

1. **The keystone** — a per-site *edge* (public domain → reverse-proxy vhost → the WP
   container) with auto-TLS. This is the missing infrastructure that blocks **#21, #15-TLS,
   #30-jail, #8-backend, and the original #25-nginx design** — and it is #34's prerequisite.
2. **Stateless WP** — shared media + shared object cache so replicas are interchangeable.
3. **Replicas + upstream LB** — multiple WP containers behind the keystone vhost.

This doc specs all three as numbered phases (`K0–K4` keystone, `H0–H4` scaling), each with
**Goal / Do / Reuse / Acceptance / Verifiable-here? / Risk**, plus a dependency graph and a
real-host verification checklist.

---

## 2. Current reality (verified against the code)

| Fact | Evidence |
|---|---|
| A managed WP site is a single `wordpress:${VERSION}-apache` container publishing `${HTTP_PORT}:80` straight to the host; per-site MySQL + per-site Redis (#23). **No domain, no per-site vhost at create.** | `backend/templates/wordpress.yaml` (`ports: "${HTTP_PORT}:80"`) |
| `site.url` falls back to the host port unless a domain is attached. | `wordpress_service.py` `_enrich_site_data` (#3) |
| **A container-proxying vhost primitive already exists**, incl. HTTPS via cert + HTTP→HTTPS redirect. | `nginx_service.py:391` `create_site(..., ssl_cert, ssl_key)`, `:450` `_with_ssl`, `:535` `add_ssl_to_site` |
| **Upstream load-balancing, `limit_req`, `proxy_cache`, and per-vhost log reads already exist** — bound to a generic `domain`/`data` dict, not to a WP site. | `nginx_advanced_service.py:31` `create_reverse_proxy` (`upstreams`→`upstream{}` at `:47`, `proxy_pass http://{upstream}` at `:95/:113`, `limit_req_zone` at `:61`, `proxy_cache_path` at `:68`), `:173` `get_vhost_logs`, `:196` `get_load_balancing_methods` |
| Wildcard cert issuance + auto-renew already exist. | `advanced_ssl_service.py:39` `issue_wildcard_cert(domain, dns_provider, credentials, email)`, `ssl_service.py:256` `setup_auto_renewal` |
| Subdomain generation for environments already exists. | `environment_domain_service.py:61` `generate_domain(production_domain, env_type, …)` |
| S3/B2/MinIO directory upload already exists (for media offload). | `storage_provider_service.py:260` `upload_directory(local_dir, remote_prefix)` |

**Conclusion:** the gap is *wiring + a public base-domain + statelessness*, not greenfield nginx
work. That makes both the keystone and #34 far smaller than "XL from zero" implies — but every
phase touches nginx apply/reload, cert issuance, or container orchestration, **none of which is
runtime-verifiable on Windows.**

---

## 3. Dependency graph

```
            ┌─────────────────────────── Part 1: KEYSTONE (per-site edge) ───────────────────────────┐
            │  K0 public base-domain + wildcard DNS/TLS                                               │
            │        │                                                                                │
            │        ▼                                                                                │
            │  K1 per-site reverse-proxy vhost ──► K2 auto-TLS per site ──► K3 preview URL (#21)      │
            │        │                                   │                                            │
            │        ▼                                   ▼                                            │
            │  K4 per-site access log + limit_req  ──► unblocks #30-jail, restores #25-nginx          │
            └────────┬────────────────────────────────────────────────────────────────────────────┘
                     │  (also closes: #15-TLS, #8-backend SSL wiring, #3 already done)
                     ▼
            ┌─────────────────────────── Part 2: #34 HORIZONTAL SCALING ─────────────────────────────┐
            │  H0 stateless WP (shared media + shared object cache)                                   │
            │        ▼                                                                                │
            │  H1 replica orchestration (internal network; edge publishes, not the container)        │
            │        ▼                                                                                │
            │  H2 upstream LB wiring ──► H3 health/rolling ──► H4 autoscale signal (optional)         │
            └────────────────────────────────────────────────────────────────────────────────────────┘
```

**Do K0→K2 first** — it alone closes #15-TLS, #8-backend, and #21 (with K3), independent of #34.
**#34 cannot start before H0**, and H0/H1 require the keystone's edge to exist (replicas can't each
publish a host port).

---

## 4. Part 1 — The keystone: per-site edge

### K0 — Panel-wide public base domain + wildcard DNS/TLS  `[M]`
- **Goal:** a configurable base domain (e.g. `apps.example.com`) with `*.apps.example.com` DNS and a wildcard cert, so *any* managed site can get a real public URL. This is the single setting whose absence makes every public-URL feature impossible today.
- **Do:** add a `base_domain` + DNS-provider binding to settings; on save, issue/refresh the wildcard cert and schedule renewal. Validate the wildcard record resolves before declaring it ready.
- **Reuse:** `advanced_ssl_service.issue_wildcard_cert` (`:39`), `ssl_service.setup_auto_renewal` (`:256`), the existing DNS-provider credentials (Connections / `provider_secret` encryption).
- **Acceptance:** with a base domain configured, `dig *.base` resolves and a valid `*.base` cert exists.
- **Verifiable here?** Config/model/validation logic yes; DNS + cert issuance **real host only**.
- **Risk:** registrar/DNS-provider variance; wildcard issuance needs DNS-01 (already supported). If no public base domain is set, the whole edge degrades gracefully to today's `localhost:PORT` behavior — keep that fallback.

### K1 — Per-site reverse-proxy vhost at create/attach  `[L]`
- **Goal:** put an nginx vhost in front of each managed WP container so it answers on a real hostname instead of `localhost:PORT`.
- **Do:** on create (and on domain-attach), call `NginxService.create_site(name, app_type='docker', domains=[hostname], port=<container published port>)` + `enable_site` + `reload`. Persist the hostname on `WordPressSite`. Make WP proxy-aware: set `WP_HOME`/`WP_SITEURL` to the hostname, trust `X-Forwarded-Proto`/`-For` in `wp-config`, and run the serialized-safe `search-replace` (#5/#9 primitive) from `localhost:PORT` → the hostname.
- **Reuse:** `nginx_service.create_site/enable_site/reload` (`:391/:483/:288`), `db_sync_service` search-replace, the `domains.py` attach flow (#8 already wires domain-attach for generic docker apps).
- **Acceptance:** a freshly created site is reachable at `http://<hostname>` and `wp-admin` loads with correct URLs (no redirect loop, no mixed `localhost` links).
- **Verifiable here?** Template rendering + the `wp-config`/search-replace logic yes; nginx apply + live HTTP **real host only**.
- **Risk:** the classic WP-behind-proxy redirect loop (needs `X-Forwarded-Proto` handling); a port is still published in K1 (internal-only comes in H1).

### K2 — Auto-TLS per site  `[M]`
- **Goal:** every site is HTTPS by default.
- **Do:** for a `*.base` subdomain, serve the K0 wildcard cert via `create_site(ssl_cert=…, ssl_key=…)` (the `_with_ssl` path already does the 80→443 redirect). For an attached custom domain, issue a per-host LE cert. Add `ssl_status` to `WordPressSite` and surface grade/expiry (the #8 `SiteSSLPanel` already renders this once a domain is routed).
- **Reuse:** `nginx_service.create_site(ssl_cert, ssl_key)` (`:391`) / `add_ssl_to_site` (`:535`), `ssl_service`/`advanced_ssl_service`, the #8 frontend.
- **Acceptance:** the site loads over HTTPS with a valid cert; HTTP redirects to HTTPS.
- **Verifiable here?** `ssl_status` model + wiring yes; cert + TLS handshake **real host only**.
- **Risk:** custom-domain certs need DNS pointed first — keep a "pending DNS" state.

### K3 — Preview URL before DNS (#21)  `[S–M, on top of K0–K2]`
- **Goal:** a working HTTPS preview link the instant a site exists, pre-DNS.
- **Do:** mint `<site|hash>.preview.<base>` via the K0 wildcard (covered by the wildcard cert, so no per-host issuance). Optionally a basic-auth gate (the env-pipeline already has basic-auth, P4). This **is** #21 once K0–K2 exist.
- **Reuse:** `environment_domain_service.generate_domain` (`:61`), K0 wildcard, `create_site`.
- **Acceptance:** every new site shows an immediately-working `https://….preview.base` link.
- **Verifiable here?** Hostname generation yes; live preview **real host only**.

### K4 — Per-site access log + `limit_req` jail (restores #25-nginx, unblocks #30-jail)  `[M]`
- **Goal:** a per-site nginx access log on the host + brute-force protection — the exact signals #25 and #30 had to defer because the container model exposes none.
- **Do:** emit a per-site `access_log` path in the K1 vhost. Add a `limit_req` zone for `wp-login.php`/`xmlrpc.php` via `create_reverse_proxy` (it already emits `limit_req_zone`/`limit_req`). Point #25's analytics at `get_vhost_logs` (a real per-vhost log) instead of/alongside the `docker logs` source it fell back to; this also restores `%D` response-time once we own the `LogFormat`.
- **Reuse:** `nginx_advanced_service.create_reverse_proxy` (`:31`, `limit_req` at `:61/:85`), `get_vhost_logs` (`:173`), `security_service` Fail2ban (optional, watching the new log).
- **Acceptance:** repeated `wp-login` hits get throttled; per-site visits/5xx/slow-pages come from the vhost log.
- **Verifiable here?** Log-parsing + zone-rendering yes; live throttling + log emission **real host only**.

**After Part 1:** #15-TLS ✅, #21 ✅, #8-backend ✅, #30-jail ✅, #25 (response-time/slow-pages) ✅ — independent of #34.

---

## 5. Part 2 — #34 horizontal scaling

> Built entirely on the keystone. The mental shift: in H1 the WP container stops publishing a
> host port; the **edge vhost (K1)** becomes the only public entry and load-balances across
> internal replicas.

### H0 — Make WP stateless  `[L]`
- **Goal:** replicas must be interchangeable — no replica-local state.
- **What's already fine:** the **object cache is already shareable** — the #23 Redis is one service in the site's compose stack, so N WP replicas pointing at the same `redis` service share it automatically. No change needed there.
- **The real statefulness is two things:**
  1. **Uploads / `wp-content/uploads`** (local disk per replica). **Decision (open):**
     - *Option A — S3/object offload* (a media-offload plugin writing to the site's configured `storage_provider`). Pro: truly stateless, scales infinitely. Con: a plugin dependency (mild tension with the WP-CLI-over-plugin thesis), egress cost, and rewriting existing media URLs. Reuse `storage_provider_service.upload_directory` (`:260`) + creds.
     - *Option B — shared volume* (NFS/CIFS or a Docker named volume shared across replicas on one host). Pro: no plugin, transparent. Con: needs shared-FS infra; single-host only unless NFS; locking edge-cases.
     - **Recommendation:** Option A for multi-host scale; Option B as a single-host MVP. Spec both; pick at execution.
  2. **On-disk page cache** — if #22 landed a *disk-writing* page-cache plugin, that's replica-local and must be moved to Redis or disabled under LB. Verify #22's cache backend before H1.
- **Acceptance:** uploading media on replica A is immediately served by replica B.
- **Verifiable here?** Offload-config + URL-rewrite logic yes; cross-replica consistency **real host only**.
- **Risk:** media migration for *existing* sites; plugin licensing for offload.

### H1 — Replica orchestration  `[L]`
- **Goal:** run N WP containers for one site.
- **Do:** change the stack so the WP service is replica-able (`deploy.replicas` under swarm, or N enumerated services / `docker compose up --scale` on a compose host) on an **internal** network — **drop the `${HTTP_PORT}:80` host publish**; only the edge nginx reaches them. This is a **breaking change to the single-container model**, so it needs a per-site opt-in + a migration path (existing sites stay single-container until "Enable scaling").
- **Reuse:** the `wordpress.yaml` template + the `_ensure_*_in_stack` additive-compose pattern (#23 redis injection is the proven precedent).
- **Acceptance:** `docker ps` shows N WP replicas for the site, none publishing a host port.
- **Verifiable here?** Template rendering yes; actual replicas **real host only**.
- **Risk:** the publish→internal flip must be coordinated with K1 (edge must exist first or the site goes dark); MySQL stays single (not replicated here — that's a separate epic).

### H2 — Upstream load-balancing wiring  `[M]`
- **Goal:** the edge vhost balances across the H1 replicas.
- **Do:** upgrade the K1 vhost to an upstream-backed one: feed the replica endpoints to `create_reverse_proxy(upstreams=[…], method=…)`. Anonymous WP is stateless → round-robin; logged-in/cart needs affinity → `ip_hash` or cookie-sticky (expose via `get_load_balancing_methods`).
- **Reuse:** `nginx_advanced_service.create_reverse_proxy` (`upstream{}` at `:47`, `proxy_pass` at `:95`), `get_load_balancing_methods` (`:196`).
- **Acceptance:** requests distribute across replicas; killing one replica doesn't drop traffic.
- **Verifiable here?** Upstream-config rendering yes; live balancing/failover **real host only**.
- **Risk:** session affinity for WooCommerce carts; cache coherence across replicas (mitigated by H0 shared Redis).

### H3 — Health checks + rolling deploys  `[M]`
- **Goal:** safe scale up/down and zero-downtime deploys.
- **Do:** per-replica health probe; drain a replica out of the upstream before stop; a scale API on the `WordPressSite`. Tie deploys (#13/#29) into a rolling pattern: update one replica, health-check, proceed.
- **Reuse:** `environment_health_service` (#26 poller), `nginx_advanced_service` (rewrite upstream on scale), #29 safe-update health gate.
- **Acceptance:** scaling and deploys cause no failed requests.
- **Verifiable here?** Orchestration logic yes; zero-downtime claim **real host only**.

### H4 — Autoscale signal (optional)  `[M]`
- **Goal:** scale on load.
- **Do:** drive H3's scale API from the #25 per-site traffic metrics (now real, via K4). Thresholds + cooldown.
- **Reuse:** #25 analytics, H3 scale API.
- **Acceptance:** sustained load adds a replica; quiet removes one.

---

## 6. Verifiable on Windows vs. needs a real Docker host

| Verifiable here (pytest / unit) | Real Linux+Docker host only |
|---|---|
| `WordPressSite` schema additions (hostname, ssl_status, scaling flags, replica count) | nginx config **apply + reload** |
| nginx **template/string rendering** (vhost, upstream, limit_req, ssl block) | Let's Encrypt / wildcard **cert issuance** |
| `wp-config` mutation + serialized search-replace logic | live HTTP / HTTPS, redirect-loop checks |
| service unit logic with mocked shell/Docker | **container replicas**, `--scale`, internal networking |
| API routes + RBAC + workspace scoping | **load-balancing / failover** behavior |
| media-offload config + URL-rewrite logic | cross-replica media + cache consistency |
| `limit_req` zone generation | actual brute-force **throttling** |

→ Each real-host item gets a **manual verification checklist** in the executing PR; nothing infra
ships claimed-working until exercised on a host (per the project's verify-before-claiming bar).

---

## 7. Sequencing recommendation

1. **K0 → K1 → K2** — the highest-value slice. Closes **#15-TLS, #8-backend**, and (with **K3**) **#21**. Ship and verify on a host *before* touching #34.
2. **K4** — closes **#30-jail** and restores **#25** response-time/slow-pages.
3. **H0 → H1 → H2** — the #34 MVP (stateless + replicas + LB). Gate H1 behind a per-site "Enable scaling" opt-in so the single-container model stays the default.
4. **H3 → H4** — production hardening + autoscale.

Each arrow is a separately shippable, separately verifiable PR. **#34's "Done when" (N replicas behind
a balancer with consistent media + cache) is reached at the end of H2.**

---

## 8. Open decisions (resolve at execution)

- **Media:** S3 offload (Option A) vs shared volume (Option B) — pick per deployment topology (multi-host ⇒ A).
- **Port model:** the K1-publishes-a-port → H1-internal-only flip is a breaking change; needs a migration for existing sites and a clear opt-in.
- **MySQL:** stays single-instance in this spec. DB replication/clustering is a distinct epic, explicitly out of scope for #34.
- **Panel vs sites:** the *panel's* single-gevent-worker constraint (agent gateway, see `CLAUDE.md`/`ARCHITECTURE.md`) is unrelated — this LB is for the **managed sites**, not the panel. Don't conflate.
- **Plugin thesis:** media-offload (Option A) and any disk→Redis page-cache move may need plugins; weigh against the WP-CLI-over-plugin preference.

---

*Cross-references: `docs/WORDPRESS_ROADMAP.md` #34, #21, #15, #8, #25, #30, #22, #23. This spec is the
"split before starting" the roadmap asks for on every `XL`.*
