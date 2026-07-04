# Platform Enhancements

A brand-neutral reference for ten capabilities that extend ServerKit's
developer-experience, team/scale, fleet, and security surfaces. Each entry
covers **what it is**, the **API endpoints** (verified against the live
blueprints), the **new models/tables**, and **how it fits** the existing
architecture — the Queue Bus job system, the Notifications layer, RBAC, and the
agent fleet.

All routes are under the `/api/v1` prefix and require a JWT
(`@jwt_required()`) unless explicitly noted. Programmatic clients may instead
present an `X-API-Key` header, in which case the key's **scopes** are enforced on
top of RBAC (see [API token scopes](#2-api-token-scopes)).

> **Verification note.** A cross-cutting smoke test,
> `backend/tests/test_enhancements_integration.py`, asserts every endpoint below
> is registered and responds sensibly. Run it with
> `cd backend && python -m pytest tests/test_enhancements_integration.py -q`.

---

## Theme: Developer experience

### 1. Container status aggregator

**What it is.** Collapses the per-container Docker states of an application
(its app/service/database containers) into **one** deterministic status using a
fixed priority hierarchy:

```
degraded > restarting > running:unhealthy > starting > running:healthy
> exited > unknown
```

The aggregation function is pure (no Docker dependency, unit-testable); the
status-fetch helpers wire it to live Docker data behind a short-TTL cache. The
aggregator never raises — with Docker unavailable it returns `unknown`.

**Endpoints** (blueprint `container_status`, mounted at `/api/v1/status`):

| Method & path | Description |
|---|---|
| `GET /api/v1/status/app/<int:app_id>` | Full aggregated status for one application. |
| `GET /api/v1/status/apps` | Lightweight summary for every app: `{ "statuses": [ {app_id, status, total, healthy}, … ] }`. |

**Real-time.** Clients emit the Socket.IO event `subscribe_container_status`;
the server pushes deltas on the `container_status` channel (only changed apps
are sent). This rides the existing Socket.IO infrastructure in
`app/sockets.py`.

**Models/tables.** None — this is a read-through aggregator over live Docker
state plus the existing `applications` table.

**Fits.** Single-status-per-app is the primitive the UI uses for status badges
without each page re-deriving Docker state.

---

### 4. Declarative template catalog

**What it is.** A declarative schema for the one-click application templates,
including auto-resolved **"magic variables"** so a template never has to declare
a generated secret/host by hand. The schema is documented in
[`TEMPLATE_CATALOG_SCHEMA.md`](TEMPLATE_CATALOG_SCHEMA.md); the endpoint below
surfaces it to the UI/editor so the supported tokens are never hardcoded.

**Endpoint** (blueprint `templates`, mounted at `/api/v1/templates`):

| Method & path | Description |
|---|---|
| `GET /api/v1/templates/catalog/schema` | The catalog schema: `schema_version`, the supported variable `type` values, and the magic-variable token catalog. |

**Variable types.** `string`, `password` (auto-generated), `port` (always
auto-assigned to a free host port), `uuid`, `random`.

**Magic variables.** Resolved at install time and persisted to `.env` /
surfaced post-install. `<NAME>` groups related tokens so the same `<NAME>`
resolves consistently within one install:

| Token | Resolves to |
|---|---|
| `${SERVICE_PASSWORD_<NAME>}` | A generated strong password, stable per `<NAME>`. |
| `${SERVICE_USER_<NAME>}` | A generated service username (`svc_<name>_<rand>`). |
| `${SERVICE_FQDN_<NAME>}` | Auto-assigned hostname (`<slug>.<base_domain>`) when an auto-domain is set; else a localhost placeholder. |
| `${SERVICE_URL_<NAME>}` | Full URL derived from the FQDN and scheme. |
| `${SERVICE_BASE64_<NAME>}` | Base64 of a freshly generated secret. |

**Models/tables.** None new for the schema endpoint — templates are
file-backed; installs continue through the existing application/template flow.

**Fits.** Template installs are dispatched as Queue Bus jobs by
`DeploymentJobService.install_template` (`POST /api/v1/templates/<id>/install`),
so a long install runs asynchronously like every other deployment.

---

### 5. Build packs

**What it is.** A transparent, zero-Dockerfile build layer. It inspects a
repository for language/framework markers (e.g. `package.json`,
`requirements.txt`, `go.mod`, `index.html`), produces a **build plan**, and
generates a `Dockerfile` + `docker-compose` from that plan. If the repo already
ships a `Dockerfile`, detection defers to it (`dockerfile-present`).

**Endpoints** (blueprint `buildpacks`, mounted at `/api/v1/buildpacks`):

| Method & path | Description |
|---|---|
| `POST /api/v1/buildpacks/detect` | Body `{repo_url?, branch?, source_connection_id?, repository_full_name?, path?}`. Clones to a throwaway workspace (or uses a supplied `path`), runs detection, returns `{plan, dockerfile, compose}`. Results are cached by `(repo_url, commit)`. |
| `POST /api/v1/buildpacks/generate` | Pure. Body `{plan, overrides?, name?}`. Returns `{plan, dockerfile, compose}` with no clone. |

A plan carries `builder`, `language`, `framework`, `versions`,
`build_command`, `start_command`, `port`, `confidence`, and `notes`.

**Models/tables.** No new table. The detected plan is persisted **on the
application row**: `applications.buildpack_type`, `applications.buildpack_plan`
(JSON), and `applications.buildpack_overrides` (JSON).

**Fits.** `detect` can resolve a clone URL through a stored source connection,
reusing the existing Connections layer; tokens are never leaked back in error
messages.

---

### 6. Deployment config snapshots

**What it is.** An immutable configuration snapshot captured before each
deployment (environment variables and domains), enabling a side-by-side diff and
a one-click restore + redeploy. Secret values are masked at snapshot time, so
listings and diffs never leak secrets.

**Endpoints** (blueprint `snapshots`, mounted at `/api/v1/apps`):

| Method & path | Description |
|---|---|
| `GET /api/v1/apps/<int:app_id>/snapshots` | List snapshots (newest first); `?limit=` (default 50, max 200). The list view omits the full payload. |
| `GET /api/v1/apps/<int:app_id>/snapshots/<int:snap_id>` | One snapshot including its resolved config. |
| `GET /api/v1/apps/<int:app_id>/snapshots/<int:snap_id>/diff` | Diff against another snapshot. `?against=<id|previous>` (default `previous`). Returns `{diff, summary, has_changes, …}`. |
| `POST /api/v1/apps/<int:app_id>/snapshots/<int:snap_id>/restore` | Restore the snapshot's config and trigger a redeploy. **Requires developer access.** |

**Models/tables.** `deployment_snapshots`
(`app/models/deployment_snapshot.py`).

**Fits.** Restore goes through `ConfigurationService.restore_snapshot`, which
re-uses the normal deployment path; RBAC gates the restore via
`@developer_required`.

---

## Theme: Team & scale

### 7. Projects & Environments

**What it is.** A grouping hierarchy layered above applications:
**Workspace → Project → Environment → Applications**. Creating a project
auto-creates a default environment. Projects are workspace-scoped; access is
derived from workspace membership (the active workspace is resolved from the
`X-Workspace-Id` header or `?workspace_id=`, falling back to the user's
accessible workspaces).

**Endpoints** (blueprints `projects` at `/api/v1/projects`, `environments` at
`/api/v1/environments`):

| Method & path | Description |
|---|---|
| `GET /api/v1/projects` | List projects across accessible workspace(s), each with resource counts. |
| `POST /api/v1/projects` | Create a project (`{name, description?, metadata?, default_environment?}`); auto-creates a default environment. Returns `201` with the project + its environments. |
| `GET /api/v1/projects/<int:project_id>` | Project with environments and counts. |
| `PUT /api/v1/projects/<int:project_id>` | Update name/description/metadata. |
| `DELETE /api/v1/projects/<int:project_id>` | Delete; refuses (`409`) if applications are still assigned. |
| `POST /api/v1/environments` | Create under a project (`{project_id, name, is_default?}`). |
| `PUT /api/v1/environments/<int:environment_id>` | Update name / default flag. |
| `DELETE /api/v1/environments/<int:environment_id>` | Delete; refuses (`409`) the project's only environment; otherwise detaches assigned apps. |
| `POST /api/v1/environments/reorder` | Reorder a project's environments (`{project_id, ordered_ids}`). |

**Models/tables.** `projects` (`app/models/project.py`), `environments`
(`app/models/environment.py`). Applications gain `applications.project_id` and
`applications.environment_id` (both nullable FKs).

**Fits.** Write access is gated by `WorkspaceService.can_write_in_workspace`,
so the existing workspace RBAC governs who may create/modify project structure.

---

### 8. Polymorphic shared resources (tags + variable groups)

**What it is.** Two cross-resource primitives:

- **Tags** — attach a label to any supported resource type and look up resources
  by tag.
- **Shared variable groups** — named groups of variables that can be *attached*
  to multiple resources; an effective, merged ("resolved") view is exposed per
  resource. Secret values are always masked in responses (group-level masking
  plus a defense-in-depth pass through `app/utils/sensitive_data_filter.py`).

**Endpoints** (blueprint `shared_resources`, mounted at `/api/v1/shared`):

| Method & path | Description |
|---|---|
| `GET /api/v1/shared/resource-types` | The catalog of supported polymorphic resource types. |
| `GET /api/v1/shared/tags` | Tags on a resource (`?resource_type=&resource_id=`) or resources by tag (`?tag=`). |
| `POST /api/v1/shared/tags` | Add a tag (`{resource_type, resource_id, tag}`). |
| `DELETE /api/v1/shared/tags` | Remove a tag (body or query params). |
| `GET /api/v1/shared/variable-groups` | List groups (optionally scoped). |
| `POST /api/v1/shared/variable-groups` | Create a group (`{scope_type, scope_id, name, description?}`). |
| `GET/PUT/DELETE /api/v1/shared/variable-groups/<int:group_id>` | Fetch (with masked variables + attachments), update, or delete a group. |
| `POST /api/v1/shared/variable-groups/<int:group_id>/variables` | Add a variable (`{key, value?, is_secret?}`). |
| `PUT/DELETE /api/v1/shared/variable-groups/<int:group_id>/variables/<int:variable_id>` | Update / delete a variable. |
| `POST /api/v1/shared/variable-groups/<int:group_id>/attach` | Attach the group to a resource (`{resource_type, resource_id}`). |
| `POST /api/v1/shared/variable-groups/<int:group_id>/detach` | Detach. |
| `GET /api/v1/shared/resolved` | Effective merged variables for a resource (`?resource_type=&resource_id=`), secrets masked. |

**Models/tables.** `resource_tags`, `shared_variable_groups`,
`shared_variables`, `shared_variable_group_attachments`
(all in `app/models/shared_resource.py`).

**Fits.** Masking reuses the central sensitive-data filter, the same primitive
used elsewhere (e.g. API-key responses), so secrets never leak through this
surface.

---

### 9. PR preview environments

**What it is.** Ephemeral preview deployments driven by pull-request webhooks.
Opening a PR provisions a preview; new commits redeploy it; closing the PR tears
it down. Per-app settings control whether previews are enabled, the preview
domain template, and a TTL.

**Endpoints** (blueprint `previews`, mounted at `/api/v1/apps`):

| Method & path | Description |
|---|---|
| `GET /api/v1/apps/<int:app_id>/previews` | List non-destroyed previews (newest PR first). |
| `GET /api/v1/apps/<int:app_id>/previews/settings` | Per-app preview settings (defaults when never configured). |
| `PUT /api/v1/apps/<int:app_id>/previews/settings` | Enable/disable + set domain template & TTL. **Developer.** |
| `POST /api/v1/apps/<int:app_id>/previews/sync` | Reconcile previews against open PRs (best-effort). **Developer.** |
| `POST /api/v1/apps/<int:app_id>/previews/<int:preview_id>/redeploy` | Re-provision for the same PR. **Developer.** |
| `DELETE /api/v1/apps/<int:app_id>/previews/<int:preview_id>` | Tear down and mark destroyed. **Developer.** |

**Webhook** (blueprint `preview_webhooks`, mounted at `/api/v1/webhooks`):

| Method & path | Description |
|---|---|
| `POST /api/v1/webhooks/pull-request/<token>` | **Public** (no JWT) — authenticated by webhook signature, like the push webhook. Maps the PR action to a preview job: opened/reopened → `preview.create`, synchronize/edited → `preview.sync`, closed → `preview.destroy`. Always returns `200` with an `action` descriptor so a harmless delivery never trips provider retries. |

The webhook recognizes the major hosted Git providers via their event headers
and reuses the **push webhook's signature verification** (same secret).

**Models/tables.** `application_previews` and `application_preview_settings`
(`app/models/application_preview.py`).

**Fits.** PR actions are enqueued as Queue Bus jobs
(`preview.create` / `preview.sync` / `preview.destroy`); handlers are registered
by `PreviewService.register_jobs()` at startup. Under the testing config the job
system is a no-op, so a webhook never fails on enqueue.

---

## Theme: Fleet

### 3. Server onboarding state machine

**What it is.** A linear lifecycle for bringing a server under management:

```
pending → validating → installing_prerequisites → installing_docker
        → pairing_agent → ready          (or → failed)
```

State advances on the Queue Bus; the panel exposes start/retry plus a status
read with an ordered progress log.

**Endpoints** (blueprint `servers`, mounted at `/api/v1/servers`; server ids are
UUID strings):

| Method & path | Description |
|---|---|
| `POST /api/v1/servers/<server_id>/onboarding/start` | Begin onboarding (`pending → validating → …`). **Developer.** |
| `POST /api/v1/servers/<server_id>/onboarding/retry` | Clear a failed onboarding and resume from validation. **Developer.** |
| `GET /api/v1/servers/<server_id>/onboarding/status` | Current state + ordered progress log. |

**Models/tables.** `server_onboarding_logs`
(`app/models/server_onboarding_log.py`) holds the authoritative step history.
The `servers` table caches the latest state on `onboarding_state`,
`onboarding_progress` (JSON snapshot), and `onboarding_updated_at`.

**Fits.** Advancement runs as the job kind `server.onboarding.advance` on the
Queue Bus (handlers registered via `ServerOnboardingService.register_jobs()`),
and the final pairing step joins the existing agent-pairing flow.

---

### 10. Per-server proxy stack

**What it is.** An opt-in, Dockerized reverse-proxy stack selectable **per
server**. Host nginx remains the default; an operator can switch a server to a
managed Traefik or Caddy stack, preview the generated compose before switching,
and (best-effort) regenerate/deploy it.

**Endpoints** (blueprint `proxy`, mounted at `/api/v1/servers`; server ids are
UUID strings):

| Method & path | Description |
|---|---|
| `GET /api/v1/servers/<server_id>/proxy` | Managed proxy stack state for a server (best-effort status). |
| `GET /api/v1/servers/<server_id>/proxy/compose-preview` | Preview the generated `docker-compose` for a proxy type without writing. `?proxy_type=traefik\|caddy\|nginx` (returns `compose: null` for nginx). Optional `?acme_email=` and `?dashboard=`. |
| `POST /api/v1/servers/<server_id>/proxy/configure` | Update `proxy_type` / `custom_snippet`; optional `deploy`. **Developer.** |
| `POST /api/v1/servers/<server_id>/proxy/regenerate` | Rewrite config and best-effort hot-reload. **Developer.** |
| `POST /api/v1/servers/<server_id>/proxy/switch` | Switch a server's proxy type (`{proxy_type}`). **Developer.** |

The generated Traefik/Caddy compose joins an external `serverkit` Docker
network, publishes ports 80/443, and (for Caddy) persists certs/config in named
volumes; ACME email is optional.

**Models/tables.** `proxy_stacks` (`app/models/proxy_stack.py`); one row per
server, defaulting to `nginx`.

**Fits.** Reads require auth; mutations require developer access, matching the
RBAC posture of the rest of the servers blueprint. The stack is per-server, so
the choice is recorded against the fleet's existing server inventory.

---

## Theme: Security & access

### 2. API token scopes

**What it is.** Fine-grained, additive scopes for API keys. Scopes are
**orthogonal to RBAC**: RBAC governs *who the user is*; a key's scopes govern
*what that key may do*. The `require_scope` decorator is a **pass-through for
JWT/session requests** (those are governed by RBAC) and only enforces scopes
when a request is authenticated with an `X-API-Key` header. A `*` master scope
grants full access; `resource:*` wildcards are honored.

**Endpoint** (blueprint `api_keys`, mounted at `/api/v1/api-keys`):

| Method & path | Description |
|---|---|
| `GET /api/v1/api-keys/scopes` | The canonical, non-empty catalog of assignable scopes. Each entry is `{key, label, group, description}`. |

The catalog includes coarse-grained scopes (`read`, `write`) and resource-scoped
entries (e.g. `apps:read`, `apps:write`, `apps:deploy`, `databases:*`,
`domains:*`, `dns:*`, `backups:*`, `servers:read`, `servers:admin`,
`secrets:read`). The full key lifecycle (`GET/POST /`, `GET/PUT/DELETE
/<id>`, `POST /<id>/rotate`) lives in the same blueprint.

**Secret masking.** Secret values in API responses are masked through
`app/utils/sensitive_data_filter.py`; a key's raw value is returned exactly once,
at creation/rotation.

**Models/tables.** `api_keys` (`app/models/api_key.py`) stores the assigned
scopes per key. The canonical scope catalog itself lives in
`app/middleware/api_scope_middleware.py` (no table).

**Fits.** `require_scope` composes with the existing auth decorators, letting one
endpoint safely serve both the web UI (JWT + RBAC) and programmatic clients
(API key + scopes). Key actions are written to the existing audit log.

---

## How these fit together

- **Queue Bus jobs.** Template installs, deployment-snapshot restores, server
  onboarding advancement, and PR-preview lifecycle all run as Queue Bus jobs —
  the same unified orchestration the rest of ServerKit uses — so long-running
  work is asynchronous and observable, and is a no-op under the testing config.
- **Notifications.** Job outcomes flow through the existing notifications layer;
  these capabilities add work to that bus rather than introducing a parallel one.
- **RBAC + scopes.** Reads require a valid JWT; mutating/privileged routes add
  `@developer_required` (snapshots restore, previews, proxy config, onboarding).
  API-key callers are additionally gated by scopes.
- **Agent fleet.** Onboarding and the per-server proxy stack are per-server and
  recorded against the existing server inventory and agent-pairing flow.
- **Secret hygiene.** Shared variable groups, config snapshots, and API-key
  responses all route secret values through the central sensitive-data filter.
