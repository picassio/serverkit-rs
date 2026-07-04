# ServerKit Redesign Map

> Status: **pass 1 COMPLETE — pass 2 (parity) IN PROGRESS (2026-06-11)** — all 22 view-groups shipped a first
> restyle on `dev`, but a fresh audit against the prototype (`_serverkit_design_extract`) found the pass-1 work
> stops short of real parity on several pages. See **§8** for the ranked gap list. Remaining: §8 parity items,
> §5 backend gaps, open §6 decisions 3/4/5/6. · Owner: Juan ·
> Source design: clickable prototype (`ServerKit.zip` → `serverkit/view_*.jsx` + `styles.css`) · Target: the live
> modular app under `frontend/src` + `backend/app`.

This is the working map for adopting the new ServerKit design across the app. It was built by comparing
**every prototype view** against its **current production page** (22 view-groups analyzed). Read §0–§1 first;
they set the thesis and the rules. §4 is the per-page checklist. §5 is the backend work. §6 is the decisions
that are yours to make.

---

## 0. What this redesign actually is

**It is a front-end re-skin + information-architecture (IA) overhaul — not a feature build.**

The prototype is a *standalone clickable mockup* (React via CDN + Babel, mock `window.DATA`). The live app
**already implements ~90% of what it shows** at the backend (62 API blueprints, 100+ services, 60+ models) and
mostly at the frontend too (45+ pages). So for most views the work is: **restyle the existing page into the new
visual language, reshuffle its IA, and slot the result into the existing modular structure** — *not* a rewrite.

A recurring trap: **the prototype is frequently a _subset_ of the live app** (Servers, Docker, Files, Git,
Security, Email all show *less* than we already ship). Taking it literally would delete working capabilities
(tabs, filters, bulk actions, file browsers, webhook management, agent fleet rails). The rule is **restyle +
selectively borrow IA, re-home what the mockup omits, never silently drop a working feature.**

---

## 1. The modularity contract (golden rules)

These keep the redesign inside the structure you already like:

1. **Remap token _values_ behind existing token _names_.** The new palette (periwinkle accent `#6d7cff`,
   3-tier surface ramp, 4-tier text, new `cyan`/`violet` semantics, `*-bg` washes) lands in
   `styles/_theme-variables.scss` + `styles/_variables.scss` by **changing values**, not renaming. Every page's
   SCSS reads `$bg-*/$text-*/$accent-*` today — a repo-wide rename is the one thing that would break everything.
   Add genuinely new tokens (`--cyan`, `--violet`, `--accent-bright`) **additively**. Keep light-theme overrides
   and the `*-raw` values used by SCSS `fade()` in sync.
2. **One feature = one page + one SCSS partial + one API module.** Restyling a page touches
   `pages/X.jsx` and `styles/pages/_x.scss` (and its `services/api/x.js` only if a new endpoint is added).
   Don't fork a new architecture.
3. **Build shared primitives once (§3), then consume them everywhere.** No per-page bespoke pills/charts/tables.
4. **Preserve the superset.** When the mockup shows fewer tabs/controls than today, re-home the missing ones
   (a drawer section, a settings sub-route, a secondary surface) — don't delete them.
5. **Keep deep-linking working.** Tabs are wired through `useTabParam`/`VALID_TABS`. If you change a tab set,
   update `VALID_TABS` and add redirects for old `?tab=` values.
6. **Bind to real `api.*`.** Drop all `window.DATA` mock wiring from the prototype; it is throwaway.
7. **One logical change per commit**, focused diffs, branch prefixes (`feature/redesign-*`).

---

## 2. Phase 0 — Design System & Chrome foundation (do this first)

Everything else depends on this. Effort: **L**. **Status: ✅ done (2026-06-07)** — tokens remapped
(dark/light/system) + prototype tokens added; IBM Plex self-hosted; accent ramp runtime-derived in
`ThemeContext`; shared primitives built in `components/ds/`; sidebar restyled (gradient rail, mono section
headers, left accent bar, dimmed icons, restyled footer/user-menu, 250px). SCSS compiles, ESLint 0 errors.
Git stays **plugin-owned** under Infrastructure (no core duplicate — avoids a dead link when the plugin is off).

- **Fonts** → IBM Plex Sans + IBM Plex Mono, **self-hosted** (OFL-licensed `.woff2` vendored in
  `frontend/public/fonts/` via `frontend/scripts/fetch-fonts.mjs`, declared in `styles/base/_fonts.scss`).
  **No Google Fonts / third-party CDN** — a self-hosted control panel must not leak every visitor's IP + usage
  to Google, and self-hosting also works on air-gapped/offline installs. `$font-main`/`$font-mono` retargeted in
  `_variables.scss`; `system-ui` fallback keeps the app working if the woff2 files are absent.
- **Tokens** → remap in `_theme-variables.scss`/`_variables.scss` per rule §1.1: surface ramp
  (`--surface`/`-2`/`-3` + near-black bg), 4-tier text (`--text`/`-dim`/`-faint`/`-ghost`), accent
  `#6d7cff` + `--accent-bright`/`--accent-dim`/`--accent-bg`/`--accent-glow`, **new** `--cyan`/`--violet`
  (+ `-bg`), radius scale (10/7/14), shadow scale, and the `.app` radial accent wash.
- **Sidebar** (`styles/layout/_sidebar.scss` + `components/Sidebar.jsx` + `components/sidebarItems.js`):
  gradient bg, 34px gradient logo tile, mono-uppercase section headers, left accent bar + glow on active item,
  optional per-item **badge dot** slot. Add a top-level **Git** item under Infrastructure; align preset
  membership to the prototype (Full / Web Hosting / Email Admin / Docker-DevOps / Minimal).
  **Keep** the current richer structure: collapsible sub-items, `WorkspaceSwitcher`, plugin-nav injection,
  dev-tools group. (Preset persistence via `sidebar_config`, theme/accent via `ThemeContext` localStorage —
  both already exist; verify the dark/light/**system** trio.)

---

## 3. Shared component library (build once → reused everywhere)

These recur across nearly every view. Build them in `components/ui/` (or `components/common/`) with styles in
new partials (`styles/components/_charts.scss`, `_datatable.scss`, `_drawer.scss`, `_kpi.scss`). **This is the
highest-leverage work in the whole redesign** — most page tasks become trivial once these exist.

| Primitive | New / Reuse | Used by | Notes |
|---|---|---|---|
| **Pill / StatusPill** | NEW | ~all | dot + label, kinds green/amber/red/gray/cyan. Replaces ad-hoc `Badge`/bespoke pills. |
| **EnvTag** | NEW | wp, services, dashboard, workspaces | PROD / DEV / STAGING / SERVICE colored tag. |
| **SegControl** | NEW | dashboard, monitoring, backups, services, security, appearance, filters | segmented toggle; replaces native `<select>`/`filter-chip`. |
| **KPI / MetricCard** | NEW | dashboard, domains, docker, git, cron, monitoring, services, wp, email, security, workspaces | icon chip + value + trend + sub. |
| **AreaChart** | NEW (consolidate `MetricsGraph`) | dashboard, servers, services, wp, monitoring, email | multi-series smooth area, gradient fill, **theme-token colors** (move hard-coded hex out). |
| **Sparkline** | NEW | kpi strips, monitors table | tiny inline polyline. |
| **DataTable (`.dtable`)** | NEW shared style | wp-list, servers, docker, databases, domains, services, cron, backups, security, email, settings | dense table; `cell-name`/`cell-mono`/`kind-ico`; selectable + bulk-toolbar variant. |
| **Ckbox (tri-state)** | NEW | selectable tables (wp-list first) | header select-all + per-row. |
| **Drawer + Scrim** | NEW — **build on existing `components/ui/sheet.jsx`** | domains, backups (restore), cron, git (repo), services (peek), infra (SSH/logs) | right slide-over; **unifies ~4 divergent drawers today** (logs-drawer bottom-dock, dx-inspector, preview-drawer, ai-drawer). |
| **svc-tile** (gradient initial avatar) | NEW | wp, services, workspaces, marketplace, db | first-letter tile, hue hashed from name (`svcGrad`). |
| **gauge** (inline mini bar) | NEW | servers, docker, services, email, wp(db), domains | thin threshold-colored fill (red>75 / amber>50). |
| **ScoreGauge** (SVG ring) | NEW | security overview, wp security posture, email reputation | donut + score/label. |
| **Heatmap** (contribution grid) | NEW | backups overview | 18-week daily intensity. |
| **feed-item** (activity feed) | NEW | dashboard, wp, security, backups, git, services | colored dot + html text + time. |
| **Switch** | **REUSE** `components/ui/switch.jsx` | settings, security, schedules, toggles | restyle to gradient-on. |
| **Icon** | **REUSE** `lucide-react` | all | do **not** add the prototype's parallel icon registry; map names. |

---

## 4. Per-page change map

Effort: S/M/L/XL. Nature: **Restyle** (look only) · **+IA** (layout/tab/route reshuffle) · **+BE** (needs new backend to be fully real).
"Update" = existing files to restyle. "Create" = genuinely new files. All paths under `frontend/src` unless noted.

| # | View | Effort | Nature | Update | Create | Backend gap (→ §5) |
|---|---|---|---|---|---|---|
| 0 | **Design system + chrome** | L | Restyle+IA | `_variables.scss`, `_theme-variables.scss`, `layout/_sidebar.scss`, `Sidebar.jsx`, `sidebarItems.js`, `index.html` | shared primitives (§3) | none (badge data minor) |
| 1 | **Dashboard** | M | Restyle+IA | `pages/Dashboard.jsx`, `MetricsGraph.jsx`, `pages/_dashboard.scss` | ServerSwitcher, ActivityFeed | activity-feed source (maybe) | **✅ pass 1 done** — metric tiles → `MetricCard`, apps status → `Pill`, chart retinted to new palette (kept the real `MetricsGraph` intact), dotsep meta. Deferred: server-switcher dropdown + activity feed. |
| 2 | **Servers list + detail** | L | Restyle+IA | `pages/Servers.jsx`, `pages/ServerDetail.jsx`, `pages/_servers.scss` | — (reuse MetricsGraph) | reboot cmd, per-server SSH launch; region/load = mock-only | **✅ top-bar migrated** — built shared `SERVER_TABS`; `PageTopbar` (Servers/Agent Fleet/Fleet Monitor/Cloud/Config Templates) added to all 5 pages; **sidebar sub-menu removed**. Main Servers page: top bar added ABOVE the ops workspace — **fleet rail / groups / workbar / bulk / pairing fully preserved** (no over-trim). Sub-page headers swapped to PageTopbar (actions kept). **List body restyled** — the existing `.servers-table` (already capable: bulk-select/group/telemetry/actions) got the demo's dense look: mono uppercase headers + thin token-colored gauges (CPU=accent-bright, RAM=cyan, Disk=green). Region/Load columns omitted (mock-only, no backend). **✅ ServerDetail tabs restyled** — header → demo detail-header (surface gradient + accent wash, mono dotsep meta, `Pill` status); overview KPI tiles → spec-card look with thin `Gauge` bars (CPU/RAM/Disk = accent-bright/cyan/green); tab strip → accent underline; data/services/packages tables → dense `sk-dtable` spec (scoped fixes over their shadcn-var partials); docker/cron/cloudflared tabs reskinned; dead blocks dropped. List/ops styles untouched. Deferred: TokenModal internals (shared with list's Add-Server modal), `serverdetail/ServicesTab.jsx` Badge→Pill (shared file), `_server-packages/_server-services.scss` proper token migration. |
| 3 | **WordPress list** | L | Restyle+IA+BE | `pages/WordPress.jsx`, `pages/_wordpress.scss` | DataTable usage | enrich `get_sites` payload (plugins/updates/visits/uptime/server/php/env); optional bulk route | **✅ top-bar migrated (LAST group — sidebar now fully flat)** — shared `WORDPRESS_TABS` (WordPress/Pipeline); `PageTopbar` on WordPress + WordPressProjects; **sidebar sub-menu removed** (Import/Create actions kept; Pipeline tab now always-visible, was wpInstalled-gated). **List body restyled** — the `.wp-site-card` grid is now a dense `.sk-dtable` (Site tile / Environments / Version / Status `Pill` / tag chips / Open-Site + WP-Admin links), with a status `SegControl` (All/Running/Stopped) + the kept tag filter. Omitted SSL/plugins/visits/uptime/server columns (not in `get_sites` — no list backend) per restyle-first. **Follow-up:** multi-select + bulk ops (needs a bulk route or per-site fan-out), list-payload enrichment. |
| 4 | **WordPress detail (tabbed)** | **XL** | Restyle+IA+BE | `pages/WordPressDetail.jsx` (owns all 14 tabs), `pages/_wordpress.scss`, `_applications.scss` | header env-switcher, FilesDrawer/code-viewer, StatCard/gauge/bar-h | DB largest-tables, OPcache/extensions, uptime response-time + 90d, analytics referrers/devices, posture score | **✅ restyled (FLAT tabs — §6.2 decided 2026-06-11)** — `VALID_TABS`/deep links byte-identical; header → demo site-header (WP tile + status dot, `Pill` + `EnvTag`, mono meta) + **real-data env-switcher** (binds `site.environments`, navigates env ids); all `app-*` overrides scoped under `.wp-detail-page` (**`_applications.scss` untouched** — Applications keeps its look until its own pass); per-tab: KPI strips, `sk-dtable` lists with update chips, hash-chip git feed, retinted REAL recharts, severity chips, `--bg-code` wells, run-row Pills. `HealthDot` fixed (referenced nonexistent `--color-success`); EnvironmentCard staging-mislabeled-DEV fixed. Deferred (§5 gaps): largest-tables, OPcache/extensions, response-time/90d, referrers/devices, posture ScoreGauge, FilesDrawer. |
| 5 | **Docker** | M | Restyle | `pages/Docker.jsx`, `pages/_docker.scss` | KPI tiles | none (strict subset of current) | **✅ pass 1 done** — added `MetricCard` KPI strip (Containers/Images/Volumes/Networks from existing `stats`) atop `dx-main`, restyled status filter → segmented control. KEPT the richer dx- rail/5-tabs/inspector/exec/multi-server. Deferred: per-container accent dot, Projects KPI, logs modal→drawer. |
| 6 | **Database Explorer** | M | Restyle | `pages/Databases.jsx`, `databases/ConsoleTab.jsx`, `SourceTree.jsx`, `ResultsGrid.jsx`, `pages/_databases.scss` | syntax-highlight/gutter editor wrapper | optional "Export as SQL" dump | **✅ pass 1 done** — page already matched the target shell (token-aligned in Phase 0); added engine status-dot glow, green result count, `UTF-8` + `● Connected` status-bar segments. Deferred: syntax-highlight editor + gutter, `+` new-tab pill, ctx-menu Browse-rows/Export-SQL, semantic cell tints. |
| 7 | **File Manager** | M | Restyle+IA | `pages/FileManager.jsx`, `file-manager/FolderTree.jsx`, `PreviewDrawer.jsx`, `ContextMenu.jsx`, `pages/_file-manager.scss` | `file-manager/highlight.js` | cloud/S3 sources = **out of scope** | **✅ top-bar migrated** — shared `FILE_TABS` (Files/FTP Server); `PageTopbar` on both pages (FileManager top bar above its fullscreen browser; FTP header swapped, conditional actions kept); **sidebar sub-menu removed**. **✅ body restyled** — SCSS-only reskin to the demo's flush three-pane `.fmgr` console (toolbar/rail/listing/status joined by 1px borders, mono eyebrow rail groups, demo tree rows, dense sticky-header list, transparent grid cards, mono status bar + crumbs); all capabilities kept (uploads/bulk/type filters/disk mounts/remote targets), zero JSX changes; dead pre-topbar blocks dropped. **✅ FTP body restyled (group complete)** — StatStrip → `MetricCard` KPI strip, users + connections → `.sk-dtable` + `Pill`, mono eyebrows, log viewer on `--bg-code`, icon-font spans → lucide (was rendering literal words). Deferred: render extra config fields the API already returns (`pasv_enable`, `max_clients`…), config-editing form (net-new surface). **Remaining follow-up:** syntax-highlight preview (`file-manager/highlight.js`), TargetPicker → demo env-switch menu (optional). |
| 8 | **Domains** | L | Restyle+IA+BE | `pages/Domains.jsx`, `pages/_domains.scss` | DomainDrawer (SSL/registration/DNSSEC/NS + inline DNS records) | registrar, registration-expiry, DNSSEC, nameservers, registration auto-renew, status (WHOIS) | **✅ migrated (first top-bar page)** — built shared **`PageTopbar`** ds primitive; Domains now uses the demo's top-bar layout with routed sub-nav (Domains/DNS Zones/SSL) replacing the **removed sidebar sub-menu**; KPI strip + segmented filter + `.sk-dtable` + detail **Drawer**. Omitted unbacked columns (registrar/DNSSEC/nameservers — no WHOIS backend). **Trio complete:** DNSZones + SSLCertificates pages now render the same `PageTopbar` (shared `DOMAIN_TABS`), so the Domains/DNS/SSL tab group persists across all three. **DNS/SSL bodies restyled** — DNS records → dense `.sk-dtable` with colored record-type chips (`.dns-rtype` per A/CNAME/MX/TXT/…) + mono content; SSL cert status → `Pill` (green Valid / amber Expiring). Domains trio now fully on the new look. |
| 9 | **Monitoring** | **XL** | Restyle+IA+BE | `pages/Monitoring.jsx`, `pages/_monitoring.scss` | `services/api/monitoring.js`, monitors/status/incidents subviews | synthetic monitors (HTTP/Ping/Port/Keyword); **status pages + incidents already have backend** — wire in; request-volume/p50-p95 per-app likely gap | **✅ top-bar migrated** — shared `MONITOR_TABS` (Monitoring/Status Pages); `PageTopbar` on both pages; **sidebar sub-menu removed** (Refresh + Start/Stop and Create-Page actions preserved). **✅ body restyled** — StatStrip → `MetricCard` KPI strip, current metrics → host-card grid (icon tile + `Pill` + thin token-colored `Gauge`s + mono host-mini footer), active alerts + history → alert-row pattern (severity dots, red-wash firing chips), rules/delivery tabs reskinned (Pills, mono eyebrows); all CRUD/scheduler/channels kept; killed the `.alert-item` collision with `_servers.scss`. Deferred (backend gaps §5): synthetic monitors, response-time/request-volume charts (page has no time-series — nothing faked), per-host fleet checks. **✅ StatusPages body restyled (group complete)** — Badges → `Pill`s, hero `comp-dots` strip (real component statuses), metrics → `sk-spec-card`, incidents → `.inc-state`/`.inc-impact` mono chips + state-tinted washes, rail visibility pills; all CRUD/check/transition flows kept. |
| 10 | **Backups** | **XL** | Restyle+IA+BE | `pages/Backups.jsx`, `pages/_backups.scss` | RestoreDrawer, Heatmap | activity timeseries, GFS retention, multi-destination storage, selective/clone restore, per-snapshot duration/progress | **✅ body restyled** — `MetricCard` KPI strip (incl. previously-unrendered `file_backups` + conditional remote card); backups → dense `sk-dtable` (type-tinted tiles, mono type chips, remote-sync `Pill`s, `SegControl` filter); schedules → dense table with cron-chip times; storage tab → real-data spec-cards + `SegControl` provider picker; restore modal → amber warn + `sk-info-row`s (stays a modal). Deferred (§5): heatmap/timeseries, GFS, multi-destination, selective/clone restore, live progress, RestoreDrawer. |
| 11 | **Git** | L | Restyle+IA+BE | `pages/Git.jsx`, `pages/_git.scss`, `services/api/files.js` | RepoDeployDrawer | provider OAuth connect/disconnect + provider list; Gitea storage/users/runners metrics | **✅ body restyled** — `MetricCard` strip (status/URL/SSH/version); repos → dense `sk-dtable` (mono branch chips, stars/forks); repo detail re-tinted (mono tree, accent hash chips); webhooks → dense table (brand tiles, `Pill`s, all actions); deployments → feed rows with status `Pill`s + rollback/redeploy; icon-font close spans → lucide. Git owns its own log-output styles now. Deferred (§5): provider OAuth cards, Gitea metrics KPIs, RepoDeployDrawer; repos/webhooks/deploys KPI counts (lazy-loaded — would fake zeros). |
| 12 | **Terminal / Logs** | L | Restyle+IA | `pages/Terminal.jsx`, `pages/_terminal.scss` | console/TargetRail, TerminalPane, LogsPane (delegate to existing consoles) | unified targets endpoint; live streaming for non-file targets | **✅ body restyled** — Log Files/Journal tabs → flush console frame (`.con-rail`-style target list, mono eyebrow groups, accent-bg active rows, log well on `--bg-code`); Processes → same frame + sk-dtable-spec sortable table with token gauges; Services cards re-tinted with status stripes; statuses → `Pill`s; Terminal now owns its `proc-chip` filter chips (dependency on `_templates.scss` `.filter-chip` removed); hashColor/priority palettes retinted. Deferred: unified target rail + live non-file streaming (backend gaps), legacy block kept for shared consumers (JournalControls/ProcessTable/ServiceCard). |
| 13 | **Services (list/new/detail)** | L | Restyle+IA+BE | `pages/Services.jsx`, `NewService.jsx`, `ServiceDetail.jsx`, `service-detail/OverviewTab.jsx`, `MetricsTab.jsx`, 3 partials | TemplateCatalog, area charts | per-service metrics history + requests/min; template-catalog source; image/scratch create paths | **✅ top-bar migrated** — shared `SERVICE_TABS` (Services/New Service/Templates/Deploy Activity); `PageTopbar` on all 4 group pages; **sidebar sub-menu removed**. NewService breadcrumb folded into the active tab; Templates search + Deployments live/refresh actions preserved. **NewService method-card chooser restyled** to the demo's centered cards (GitHub OAuth + manifest flows kept). **Services list body restyled** — StatStrip → `MetricCard` KPI strip (Running/Stopped/Total/Last Deploy), status → `Pill`, mono dense list header + surface tokens; bulk-select/filters/sort/per-row actions/repo-pill all kept. **✅ ServiceDetail tabs restyled** — header → `ServiceTile` avatar + `Pill` + mono dotsep meta (+ real domain link); Overview → `MetricCard` strip + `sk-info-row` + `EnvTag` + `Gauge`s, deploy statuses → `Pill`s; Metrics gauges tokenized (no fake history — backend gap); Packages → `sk-dtable`; Events → tone dots; logs/shell/commands/gunicorn consoles on `--bg-code` (fixes light theme); settings/git-modal via SCSS. **✅ Templates catalog restyled** — `.tpl-card` grid (icon tile, mono version, clamped desc, tag chips, amber featured star, hover-reveal Deploy wired to existing install flow) + cat-chip category filter; drawer/modal stacking fixed. Deferred: AreaChart history (backend gap), Environment tab (shared `EnvironmentVariables`), per-chip category counts (endpoint returns names only), `utils/serviceTypes.js` old-palette hex retint (shared — also feeds ApplicationDetail/Deployments). **Remaining follow-up:** Docker-Image/From-Scratch create methods (backend gap). |
| 14 | **Cron Jobs** | L | Restyle+IA+BE | `pages/CronJobs.jsx`, `pages/_cron.scss`, `services/api/system.js` | CronDrawer | run history, next-run, success-rate, job target metadata | **✅ body restyled** — StatCards → `MetricCard` KPI strip (real status), job cards → dense `.sk-dtable` (mono schedule chip + readable hint, `Pill` status, all row actions incl. run-now), list head with search + All/Enabled/Disabled `SegControl` (client-side), modals reskinned (`--bg-code` run output). Deferred: next-run/last-status/duration/success-rate columns + CronDrawer (backend gaps). |
| 15 | **Security (suite)** | L | Restyle+IA+BE | `pages/Security.jsx`, all `components/security/*Tab.jsx`, `pages/_security.scss`, `services/api/security.js` | ScoreGauge usage | geo-blocking, policy toggles, scan schedules, security sessions, pending-update list, integrity verify/revert. **fail2ban + SSH-keys already exist** (minor missing fields) | **✅ suite restyled (shell + 13 tabs)** — StatStrip → `MetricCard` KPI strip; tab strip → accent wash (scoped SCSS, ids untouched); finding/ban/key/rule lists → dense `sk-dtable`s with mono cells + state chips; Lynis `hardening_index` + audit `score` rendered via **`ScoreGauge` (real data)**; Integrity → kind chips + `--bg-code` file lists; Firewall sub-views → `SegControl`; Events → ds `Feed`. All flows kept. Deferred: overview posture score (no backend number), geo-blocking/policy toggles/schedules/sessions/pending-updates/verify-revert (§5). Note: `.sec-state` is the 3rd copy of the mono state-chip pattern → candidate `.sk-state` primitive. |
| 16 | **Workspaces (list + detail)** | L | Restyle+IA+BE | `pages/Workspaces.jsx`, `pages/_workspaces.scss`, `services/api/servers.js` | **`pages/WorkspaceDetail.jsx`** + `/workspaces/:id` route | **plan/billing**, aggregated CPU/Mem/Storage usage, service/site counts, member `last_active` | **✅ list restyled** — header → `PageTopbar`; cards → prototype `.ws-card` (ServiceTile avatar honoring real `primary_color`, status `Pill`, mono slug/since meta, stat strip, hover lift); members modal → avatars + role Pill/tags (**backend role names kept** — §6.6 unresolved); sharing ACL select → `SegControl`. Deferred: plan/billing/usage/counts (no payload), WorkspaceDetail page (net-new). |
| 17 | **Marketplace** | M | Restyle+IA | `pages/Marketplace.jsx`, `pages/_marketplace.scss` | — | 3rd-party integration connect (OAuth) + "core" flag | **✅ pass 1 done** — SCSS-only: hero/section kickers → mono eyebrow, card hover shadow → token, category filter active → accent chip (cards already had hover-lift + tinted tiles). PRESERVED tabs + plugin-install-from-source + side panel (no regression). **Also top-bar migrated:** shared `MARKET_TABS` (Marketplace/Downloads); `PageTopbar` replaced the hero; **sidebar sub-menu removed** (Import ZIP action kept). Deferred: single-scroll IA + 3rd-party integrations (needs OAuth backend). |
| 18 | **Email Server** | L | Restyle+IA+BE | `pages/Email.jsx`, `pages/_email.scss`, `services/api/system.js` | KPI/gauge/reputation-ring | mail-volume timeseries, reputation/RBL, structured activity feed, real mailbox storage, per-item retry | **✅ body restyled** — service cards → icon tiles + `Pill`s (controls kept); Domains tab → real-data `MetricCard` KPI strip + dense `sk-dtable` with DKIM/SPF/DMARC presence pills; Accounts/Aliases/Forwarding → dense tables (mono cells); queue → dense table now rendering the already-loaded Recipients field; previously-unstyled password modal fixed. All scoped under `.email-page`. Deferred: volume timeseries/reputation ring/activity feed/mailbox storage/retry (§5). Follow-up: `_email.scss` still hosts global `.status-badge`/`.empty-state` blocks other pages consume — consolidate into component partials in a later sweep. |
| 19 | **Workflow Builder** | M | Restyle+IA | `pages/WorkflowBuilder.jsx`, `pages/_workflow.scss` | wf-rail / wf-head regions (reuse ReactFlow nodes) | ~~workflow-level active/paused flag (likely)~~ **resolved: `Workflow.is_active` already exists end-to-end** | **✅ restyled** — toolbar/palette cards → flush wf-head (ghost title input, real `is_active` `Pill`, mono meta) + wf-rail palette (eyebrow groups, tinted node chips); **Active switch wired to the real flag**; ReactFlow chrome tokenized (theme-aware dots, surface minimap/controls, accent edges) with node geometry kept byte-identical; **WorkflowExecutionHistory `gray-*` Tailwind theming bug fixed** (token console frame, status `Pill`s); node/panel palettes retinted. Note: `utils/connectionRules.js` `connectionColors` export now consumer-less — cleanup candidate. |
| 20 | **Settings** | L | Restyle | `pages/Settings.jsx` (IA already matches!), all `components/settings/*Tab.jsx`, `pages/_settings.scss` | — | **active-session list + revoke** (JWT JTI registry) — only real gap | **✅ pass 1 done** — nav group labels → mono eyebrow, toggles → gradient-on (reskins all 17 tabs), theme-preview swatches repainted to new palette. Deferred: per-tab `.dtable`/Active-Sessions. |
| 21 | **Infra overlays (SSH + log drawers)** | M | Restyle+IA | `components/RemoteTerminal.jsx`, `LogsDrawer.jsx`, `wordpress/ContainerLogs.jsx`, `_logs-drawer.scss` | `styles/components/_drawer.scss` | structured `{ts,level,msg}` log payloads (optional; can parse client-side) | **✅ restyled (bottom-dock kept — §6.3 still open)** — LogsDrawer → sk-drawer head + `--bg-code` well + mobile offset fix; RemoteTerminal was literally unstyled (embedded VS Code `<style>` tag) → proper token home in `_logs-drawer.scss`, xterm theme retinted (+ v5 `selectionBackground` key fix); ContainerLogs → tile header (styles live in `_wordpress-pipeline.scss`). Deferred: structured log parsing, xterm fontFamily → IBM Plex Mono, an up-facing `--shadow-lg-up` token (two literal shadows want it). Note: RemoteTerminal is unmounted anywhere (per-server SSH launch = §5 gap) — chrome is ready when a surface hosts it. |

### Notes on the nuanced ones
- **WP detail (#4):** the single biggest task — 14 tabs. The prototype merges some into a **Security hub**
  (Posture/Vulns/Updates) and a **Settings hub** (General/PHP/Git/Uptime/Reports), and turns Files/Logs into
  **drawers**. Adopting hubs changes `VALID_TABS` deep-linking → see decision §6. Recommended: keep tabs flat,
  restyle in place, add the header env-switcher + Files drawer.
- **Monitoring (#9) / Backups (#10):** these read as a *different, larger product surface* than today's
  resource-threshold monitor / CRUD backup page. Treating them as pure restyle would silently drop Alert
  Rules / Delivery / History (monitoring) and Settings/retention (backups). Several regions are net-new backend.
  Sequence these **last** and decide build-vs-stub (§6).
- **Servers (#2), Docker (#5), Files (#7), Git (#11), Security (#15), Email (#18):** prototype is a subset.
  Keep the agent-fleet rail / 5 docker tabs+inspector+exec / file type-filters+disk-mounts / git webhooks+deploy
  history+file browser / security audit tab / email aliases+forwarding+per-service controls. Re-home, don't drop.

---

## 5. Backend gap register

Most of the redesign needs **no backend work**. These are the genuine gaps — each is "build it" only if you want
that surface to be *live* rather than stubbed. Recommended default: **restyle first with the data we have, ship
net-new backend behind the new UI in follow-ups.**

**Net-new concepts (largest):**
- **Workspace plan/billing** — plan tier (Free/Starter/Team/Business), price, seats, invoices. No model today
  (only `billing_notes`). Plus aggregated per-workspace CPU/Mem/Storage usage, service/site counts, member `last_active`.
- **Monitoring** — synthetic uptime monitors (HTTP/Ping/Port/Keyword + scheduler + history + multi-region) and
  per-app request-volume / p50-p95. *Status pages + incidents + uptime already have backend* (`status_pages_bp`,
  `uptime_bp`, `StatusIncident`) — wire them into the new Monitoring IA rather than rebuild.
- **Backups** — activity timeseries/heatmap, GFS retention (daily/weekly/monthly), multiple named storage
  destinations, selective/clone restore + pre-restore safety snapshot, per-snapshot duration & live progress.

**Field/endpoint additions (medium):**
- **Cron** — persist run history + compute next-run + success-rate + job target metadata.
- **Security** — geo-blocking; policy toggles (enforce-2FA, auto-ban, block-on-critical-malware, login-alerts);
  scan schedules; security-scoped active sessions; pending-update package list; integrity verify/revert state.
- **Settings → Security** — active session listing + revoke (needs a JWT JTI/token registry).
- **Git** — provider OAuth connect/disconnect + provider list; Gitea storage/users/orgs/uptime/runners metrics.
- **Domains** — registrar, registration expiry, DNSSEC, nameservers, registration auto-renew, status (WHOIS-backed).
- **Services** — per-service metrics history (CPU/Mem/Net/Requests over ranges) + requests/min; real template
  catalog source; first-class Docker-image / from-scratch create paths.
- **WP detail** — DB largest-tables, OPcache + extensions toggles, uptime response-time + 90-day rollup,
  analytics referrers/devices, computed posture score.
- **Email** — mail-volume timeseries, sender reputation/RBL checks, structured delivery activity feed, real
  per-mailbox storage usage, per-message retry.
- **Marketplace** — third-party integration connect (OAuth/token store) + a `core` flag on catalog entries.
- **Workflow** — workflow-level active/paused enable flag (verify `updateWorkflow` doesn't already persist one).
- **WP list / Dashboard** — enriched `get_sites` list payload; an audit/activity events source for the feeds.
- **Console** — a unified "targets" aggregator endpoint; push-based live log streaming for non-file targets.

**Out of scope (recommend defer/skip):**
- File Manager **cloud/S3 "storage sources"** (current sources are panel host + remote agents).
- Server **region** and **load-average** columns (no field in the server model; presentational only).

---

## 6. Decisions that are yours to make

**Decided 2026-06-07:** (1) **Net-new backend = restyle-first, stub the rest** — reskin every page now with
existing data; render net-new regions with clearly-stubbed placeholders; build backend gaps in follow-ups.
(2) **Start = Phase 0 foundation.**
(3) **Infra pages adopt the demo's top-bar layout, NOT sidebar sub-menus.** In the demo, Servers/Domains/Services
have no expanded sidebar sub-tree — each page carries its own **top bar** (icon + title + spacer + actions), and
detail pages use breadcrumb + in-page tabs. So as each infra page is migrated, **remove its sidebar `subItems`**
(`sidebarItems.js`) and move that navigation into a page-level top bar / in-page nav. Build a reusable page
top-bar component (matching the demo's `.topbar`) when the first such page is migrated. The app keeps ALL its
extra capabilities (Agent Fleet, DNS Zones, SSL, FTP, Status Pages, etc.) — they move from the sidebar into the
page's top bar / sub-nav, they are NOT removed. **When Domains and Servers are migrated they should look almost
identical to the demo.**
(4) **Logo = the demo's gradient tile** (periwinkle gradient rounded-square + white server glyph). Applied to the
sidebar brand; `ServerKitLogo` (the detailed SVG mark) still appears on login/setup/about/mobile — propagate the
tile there only if desired.

Remaining IA forks (pick before the dependent page starts):

1. ~~**Net-new backend policy**~~ — **decided: restyle-first, stub the rest.**
2. ~~**WP detail tabs**~~ — **decided 2026-06-11: keep 14 flat tabs** (restyled in place, deep-links preserved).
3. **Logs surface** — keep the current bottom-dock `LogsDrawer`, or move to the prototype's right-side drawer?
4. **Dashboard** — keep widget show/hide/reorder (`useDashboardLayout`) or the prototype's fixed 3-band layout?
5. **Marketplace** — keep the plugin-install-from-source UI (URL/folder/zip) as a secondary surface (recommended) or drop it?
6. **Workspace roles** — backend is `owner/admin/member/viewer`; prototype shows `Owner/Admin/Developer`. Map labels or rename?

---

## 7. Recommended sequencing

- **Phase 0 — Foundation:** §2 design system + chrome, and the §3 shared primitives. Nothing ships convincingly
  until these exist.
- **Phase 1 — Restyle-only wins (validate the system, no backend):** Settings (#20, IA already matches),
  Databases (#6), Docker (#5), Marketplace (#17), Dashboard (#1), Workflow (#19), infra drawers (#21).
- **Phase 2 — Restyle + IA (subset-preserving):** Servers (#2), WP list (#3), Services (#13), Git (#11),
  Terminal (#12), Cron (#14), Domains (#8), Email (#18), Security (#15), Workspaces list (#16).
- **Phase 3 — XL + backend-dependent:** WP detail (#4), Monitoring (#9), Backups (#10), Workspace detail +
  billing (#16). Gate each on the §6 decisions and §5 backend choices.

Each page is one (or a few) focused commit(s): restyle the page file + its SCSS partial, consume shared
primitives, add API methods only where a §5 gap is being filled.

---

## 8. Parity pass 2 — gap audit (2026-06-11, vs `C:\Users\Juan\Downloads\_serverkit_design_extract`)

Pass 1 shipped, but a per-page re-audit against the prototype shows several pages still *read* as the old design.
Root causes recur: (a) **cardified layouts** where the demo is a flush full-height console, (b) **deferred items
that carry most of the visual identity** (SQL highlighting, semantic tints, src-badges), (c) **two pages never
migrated at all** (Agent Fleet, Fleet Monitor). Ranked by how loudly each page diverges:

### 8.0 Chrome — Agent Fleet / Fleet Monitor (the "page reloads" complaint) — **WORST**
Navigation is SPA-clean (`PageTopbar`→`NavLink`); the jarring jump from `/servers` → `/fleet` is purely visual:
`AgentFleet.jsx` (39 hits) + `FleetMonitor.jsx` (24 hits) still use hard-coded Tailwind light-mode utilities
(`bg-gray-200`, `bg-yellow-50`, `text-gray-600`, `bg-blue-600`, `bg-green-600`) + legacy `StatCard`/`StatsGrid`
+ shadcn `Tabs`, so they render as a different app (and break dark theme). Fix: retint to tokens, `StatCard`→
`MetricCard`, statuses→`Pill`, tab strip→accent underline. (`Dashboard.jsx` has 1 stray `gray-*` too.)

### 8.1 Databases — layout model is wrong (user's #1 complaint)
Demo is a **flush, full-height, zero-padding 3-pane console** (topbar / tree-rail | main / statusbar joined by
1px borders). Live `.db-explorer` is **padded floating cards** (`$space-3` padding + gaps + `$radius-lg` on
toolbar/tree-panel/workspace/statusbar) — this alone makes it "not the demo". Then the deferred items:
- **De-cardify** (STYLE): `_databases.scss` — `.db-explorer` (≈:14) padding/gap→0; `.dbx-toolbar` (≈:90),
  `.dbx-tree-panel` (≈:187), `.dbx-workspace` (≈:377), `.dbx-statusbar` (≈:853) → borders-only, radius 0.
- ~~**SQL editor**~~ ✅ **done 2026-06-11**: `databases/SqlEditor.jsx` — transparent-textarea-over-highlighted-pre
  overlay with line-number gutter + SQL keyword/string/number/comment tints (`.dbx-sqled`); also **click-to-open
  UX fixed** — single-clicking a database node now opens its SQL console (was context-menu-only) and expands its
  tables; collapse stays on the chevron.
- **Semantic cell tints** (JSX): port demo `cellEl` (`view_databases.jsx:24–31`) into `ResultsGrid.jsx` —
  NULL ghost-italic, emails/URLs green, publish/running green, draft/stopped amber.
- Small: tree status-dot glow (`box-shadow: 0 0 6px currentColor`), `+` new-tab pill, ctx-menu "Browse rows"
  / "Export as SQL", bold green row count.

### 8.2 Services list — ✅ **list done 2026-06-11**
List restyled to the demo: flex rows → dense `.sk-dtable` (ck / Service tile+type / **Source badge**
(GitHub/git icon + owner/name from `deploy_repo_url`) / Domain / Status `Pill` (amber deploying·building, red
failed) / Last Deploy / Actions with lucide icons); status `<select>` → `SegControl` (Type + Sort selects kept —
capability superset); bulk-select/row-select kept via `is-selected`. Omitted CPU/Memory/Server columns (no list
payload — §5). Remaining in this group:
- Detail: no back-link ("‹ All services"), title row stacks instead of inline h1+Pill (LOW).
- NewService: demo's 4 method cards vs live 3 (Docker-image + scratch = §5 backend gap); 2-col review-panel
  layout is a deliberate improvement — keep.
- Backend (§5): metrics-history AreaCharts, real template catalog (live has 1 hardcoded template).

### 8.3 Terminal / Console — ✅ **Terminal tab shipped 2026-06-11** (rail IA still §5)
The Terminal page now has an actual terminal: a **Terminal tab** (`.term-shell`: demo-style target rail with
glowing status dots + xterm pane) hosting `RemoteTerminal` against paired agent servers. This required fixing
the whole output path — `RemoteTerminal` referenced a **nonexistent `SocketContext`** and listened for a
`terminal:<sid>` event nothing emits; rewired to `services/socket.js` + new `subscribe_terminal` /
`unsubscribe_terminal` handlers in `sockets.py` that join the browser into the gateway's
`server_<id>_terminal:<session>` stream room (agent PTY → gateway `server_stream` → xterm), with
reconnect re-subscribe. 'logs' stays the default landing (shell needs a connected agent; panel-host PTY =
§5 gap, local is rejected by TerminalService). Still §5: the unified all-targets rail (WP/DBs/containers
groups) + live non-file log streaming.
**Verified live end-to-end 2026-06-12** (WSL agent → panel → browser xterm: `echo` round-trip clean). Fixes
shipped while testing: **keystroke serialization** in RemoteTerminal (each key was its own HTTP POST — parallel
POSTs reordered fast typing into scrambled input; now queued + flushed sequentially); page header → `PageTopbar`;
rail rows got server icons + right-aligned glowing dots; shell pane flush against the rail (demo con-main).
Agent repo fixes found en route: **Linux build was broken** (`agentui/pairer.go` called Windows-only
`isServiceInstalled`/`waitForServiceStopped` — stubs added to `platform_other.go`) and **`register` panicked**
(`-c` shorthand collision with global `--config`). NOTE: agent PTY is Unix-only — Windows agents return
"pty: unsupported" (ConPTY = future agent work).

### 8.4 WordPress — ✅ **JSX-only parts done 2026-06-11**
- List: **bulk-select shipped** — header + per-row checkboxes, accent `is-selected` rows, bulkbar with
  Start/Stop/Update-core/Backup/Clear-cache fanned out per site over the real APIs (unarchive/archive/
  updateCore/createSnapshot/flushCache), each scoped by an `applies` filter (e.g. Stop only hits running
  sites). Restart/bulk-Delete skipped (no site-level restart API; delete too destructive for bulk).
- Detail: **posture ScoreGauge shipped** in the Security tab — score computed client-side from 3 real signals
  (integrity clean, WP_DEBUG off, no critical/high vulns) with a checklist; unrun checks stay out of the
  score. **PHP version added to the header subtitle** (`site.application.php_version`).
- Still §5 (data missing): Plugins/Visits/Uptime/Server/Env list columns (`get_sites` enrichment),
  largest-tables + gauge bars (no table-size endpoint), response-time chart, referrers/devices bars,
  FilesDrawer w/ code viewer.

### 8.5 Files — ✅ **done 2026-06-11**
Shipped: **`file-manager/highlight.js`** (the demo's `hlCode` line tinting — kw/str/num/cmt/var tokens; PHP/JS
keywords; escapes before inserting markup) rendered as `.pv-code` (line-number gutter on `--bg-code`) in the
PreviewDrawer's read view — editing keeps the textarea, files >4000 lines fall back to plain; **Quick-access
rail group** (Sites `/var/www`, Stack `/opt/serverkit`, Web config `/etc/nginx`, Logs `/var/log`); topbar
`meta` shows the browse target (remote agent name / "panel host"); breadcrumb `›` icons → demo mono `/`;
ext-badge on image thumbs now always visible.

### 8.6 Docker — ✅ **done 2026-06-11** (2 of 4 items were non-issues)
KPI icon tone-wash: **false alarm** — verified `.sk-kpi__icon--*` rules + `tone` props are correct end-to-end.
**Per-container accent dot shipped**: hue-hashed identity dot in the name cell (demo `.dot-ico`), dimmed when
stopped (status lives in the Status pill). Projects KPI: **skipped** — compose-project labels only exist on
per-container inspect, a count needs an extra fetch per target, and the 4th KPI slot already carries real
Networks data. Logs modal→drawer stays blocked on §6.3.

### 8.7 Workspaces — ✅ **done 2026-06-11 (commit 2312cf4)**
Built `/workspaces/:id` **WorkspaceDetail** on real data: site-header hero (brand tile, active/status Pill, mono
meta), Workspace info card + live Resources counts, accent-underline tabs (Servers/Services/Sites/Members) over
`sk-dtable` rows with Manage links + move-in/out pickers; member add/remove + per-app Share ACL re-homed from the
old list modals; Set active (mirrors WorkspaceSwitcher localStorage scope), Settings edit (updateWorkspace),
Archive/Restore/Delete. List cards now navigate to detail (demo "Open ›" footer) and the scoped workspace gets the
`.ws-card.active` accent wash. Still §5-deferred: plan/billing, aggregated CPU/MEM usage bars, member `last_active`.

### 8.8 Marketplace — effectively done
Matches the demo's spirit; intentional IA deltas (tabs, sidebar categories) preserve live capabilities.
Optional: horizontal `.cat-chips` row above the grid, 3rd-party integration cards (§5 OAuth).

### Recommended order
1. **8.0 Agent Fleet + Fleet Monitor retint** — kills the "different app" jump. ✅ **done 2026-06-11**
2. **8.1 Databases de-cardify + tints + editor** — the called-out page; mostly SCSS + small JSX. ✅ **flush
   layout + cell tints + dot glow + `+` pill + ctx items done 2026-06-11; SQL gutter/highlight follow-up**
3. **8.2 Services list → `.sk-dtable` + `SegControl` + src-badge.**
4. **8.4 WP list bulk-select + detail ScoreGauge/gauges** (JSX-only parts).
5. **8.5 Files highlight.js + quick-access.** 6. **8.6 Docker smalls.** 7. **8.3 Terminal IA** (needs §5).
8. ~~**8.7 WorkspaceDetail**~~ ✅ **done 2026-06-11** (plan/billing/usage still §5). 9. **8.8 Marketplace optionals.**

---

*Generated from a 22-agent comparison of the prototype vs the live codebase, re-audited 2026-06-11 (§8).
Update this file as decisions land and pages ship.*
