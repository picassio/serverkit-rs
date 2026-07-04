# Template Catalog Schema

This document describes the declarative service-template format ServerKit uses
for one-click application deployment, and the **magic-variable** convention that
lets template authors use industry-standard placeholders without declaring an
explicit variable for every generated secret.

Templates are plain YAML files. The bundled catalog lives in
`backend/templates/*.yaml` (≈105 templates), indexed by `backend/templates/index.json`.
A remote/community repository serves the same shape at `<repo>/index.json` plus
`<repo>/templates/<id>.yaml`, fetched by the panel's repository sync.

- Parser / validator / installer: `backend/app/services/template_service.py` (`TemplateService`)
- HTTP surface: `backend/app/api/templates.py` (`/api/v1/templates`)
- Live, machine-readable description of variable types + magic tokens:
  `GET /api/v1/templates/catalog/schema`

---

## 1. Template document

A template is a single YAML mapping. Fields:

| Field | Required | Type | Notes |
|-------|----------|------|-------|
| `id` | recommended | string (slug) | Lowercase `a-z 0-9 -`. Should match the filename stem (`ghost.yaml` → `ghost`). |
| `name` | **yes** | string | Human-readable display name. |
| `version` | **yes** | string | Free-form version (quote it so YAML keeps it a string, e.g. `"5.75"`). |
| `description` | **yes** | string | Short one-line summary. |
| `icon` | recommended | string | URL or inline `data:image/svg+xml;base64,...` (offline-safe). |
| `categories` | no | list[string] | Used for browse/filter (e.g. `cms`, `database`). |
| `website` | no | string | Project homepage. |
| `documentation` | no | string | Docs URL. |
| `variables` | no | list **or** dict | See [§2](#2-variables). |
| `compose` | **yes\*** | mapping | A Docker Compose document (`services:` required). |
| `dockerfile` | **yes\*** | string | Alternative to `compose`. |
| `files` | no | list | Config files to materialize + bind-mount. See [§4](#4-files). |
| `scripts` | no | mapping | `pre_install` / `post_install` / `pre_update` / `post_update` shell scripts. |
| `ports` | no | list | Documentation of exposed ports (display only). |
| `requirements` | no | mapping | e.g. `memory: 512MB`, `storage: 2GB` (display only). |
| `auto_domain` | no | bool | When `true`, the installer publishes the app at `<slug>.<base_domain>` if a managed-sites base domain is configured. Best-effort, never forces SSL. |

\* A template must have **either** `compose` **or** `dockerfile`. Validation
lives in `TemplateService.validate_template`.

### Minimal example

```yaml
id: my-service
name: My Service
version: "1.0"
description: Example service
icon: https://example.com/icon.png
categories:
  - utility

variables:
  - name: HTTP_PORT
    type: port
    default: "8080"
    description: Port to expose the service

compose:
  services:
    app:
      image: example/app:latest
      container_name: ${APP_NAME}
      restart: unless-stopped
      ports:
        - "${HTTP_PORT}:8080"
```

---

## 2. Variables

`variables:` may be written in **either** form; the installer normalizes both:

```yaml
# List form (preferred for new templates)
variables:
  - name: DB_PASSWORD
    type: password
    length: 24
    description: Database password
    hidden: true

# Dict form (legacy, still supported)
variables:
  DB_PASSWORD:
    type: password
    length: 24
```

### Variable fields

| Field | Notes |
|-------|-------|
| `name` | **Required.** Referenced in the template as `${NAME}`. Convention: `UPPER_SNAKE_CASE`. |
| `type` | One of the [types below](#variable-types). Defaults to `string`. |
| `default` | Default value (used as the seed for `port`, fallback for `string`). |
| `description` | Shown in the install UI. |
| `required` | If `true` and not auto-generated, the user must supply it. |
| `hidden` | Hide from the install form (auto-generated secrets / ports). |
| `length` | For `password` / `random`. |
| `special_chars` | For `password`: include `!@#$%^&*` when `true`. |
| `options` | Suggested choices (display hint). |

### Variable types

Resolved by `TemplateService.generate_value`:

| Type | Auto-generated | Behavior |
|------|----------------|----------|
| `string` | no | Uses `default`; if `required` and unset, the user must provide it. |
| `password` | yes | Strong secret of `length` chars (default 32); adds special chars when `special_chars: true`. |
| `port` | yes | **Always** auto-assigned to a free host port (DB + Docker + bind-test checked). Never taken from user input. A global `managed_app_base_port` setting overrides the per-template seed. |
| `uuid` | yes | UUIDv4. |
| `random` | yes | Random hex token of `length` chars. |

> The API marks `port`, `password`, `random`, and `uuid` as `auto_generated`
> (and `port` as `hidden`) when describing a template to the UI.

### Substitution

Anywhere in `compose`, `files[].content`, and `scripts`, the token `${NAME}`
is replaced with the resolved variable value (`TemplateService.substitute_variables`,
pattern `\$\{([A-Z_][A-Z0-9_]*)\}`). `${APP_NAME}` is always available (the
chosen app name). All resolved variables are written to the app's `.env`.

---

## 3. Magic variables

Magic variables are auto-resolved placeholders that need **no `variables:`
entry**. They let authors use industry-standard tokens for generated secrets,
service users, and the app's public address. They are resolved at install time
and merged into the install variables (so they render via the normal `${...}`
substitution, land in `.env`, and can be surfaced post-install).

Resolution is implemented in `TemplateService.resolve_magic_variables` /
`collect_magic_variables` and is **pure and unit-testable** — no Docker, no
network. The only contextual input is an optional `context`
(`app_name` / `fqdn` / `scheme`).

### Supported tokens

Written as `${SERVICE_<KIND>_<NAME>}`, where `<NAME>` is an author-chosen
identifier (`UPPER_SNAKE_CASE`) that **groups** related tokens — the same
`<NAME>` resolves to the *same value* throughout one install.

| Token | Resolves to |
|-------|-------------|
| `${SERVICE_PASSWORD_<NAME>}` | A generated strong password (32 alphanumeric chars; compose/shell-safe). |
| `${SERVICE_USER_<NAME>}` | A generated service username: `svc_<name>_<rand>` (lowercased, `[a-z0-9_]`). |
| `${SERVICE_FQDN_<NAME>}` | The app's auto-assigned hostname `<slug>.<base_domain>` when the template sets `auto_domain: true` and a base domain is configured; otherwise a `localhost` placeholder the finalizer can fill in later. |
| `${SERVICE_URL_<NAME>}` | Full URL derived from the FQDN and scheme (`https` when wildcard HTTPS covers the host, else `http`). |
| `${SERVICE_BASE64_<NAME>}` | Base64 of a freshly generated secret. |

> **Stability:** within a single install, each unique token is generated **once**.
> Re-using `${SERVICE_PASSWORD_DB}` in five places yields five copies of the
> same password. `${SERVICE_PASSWORD_DB}` and `${SERVICE_PASSWORD_CACHE}` get
> different values.

> **Best-effort & non-fatal:** FQDN/URL resolution degrades gracefully. If site
> routing isn't configured (or the template doesn't opt into `auto_domain`),
> these fall back to a documented `localhost`-style placeholder instead of
> failing the install.

### Example

```yaml
compose:
  services:
    app:
      image: example/app:latest
      container_name: ${APP_NAME}
      environment:
        APP_URL: "${SERVICE_URL_WEB}"
        DB_USER: "${SERVICE_USER_DB}"
        DB_PASSWORD: "${SERVICE_PASSWORD_DB}"
        SESSION_SECRET: "${SERVICE_BASE64_SESSION}"
    db:
      image: postgres:16
      environment:
        POSTGRES_USER: "${SERVICE_USER_DB}"     # same value as above
        POSTGRES_PASSWORD: "${SERVICE_PASSWORD_DB}"
```

### Equivalence with declared variables

Magic variables are sugar over the existing generated-variable machinery. The
classic, fully-declared form — as used today by, for example,
`backend/templates/ghost.yaml`:

```yaml
variables:
  - name: DB_PASSWORD
    type: password
    length: 24
    hidden: true

compose:
  services:
    ghost-db:
      environment:
        MYSQL_ROOT_PASSWORD: "${DB_PASSWORD}"
        MYSQL_PASSWORD: "${DB_PASSWORD}"
```

…is equivalent to the magic-variable form (no `variables:` entry needed for the
secret):

```yaml
compose:
  services:
    ghost-db:
      environment:
        MYSQL_ROOT_PASSWORD: "${SERVICE_PASSWORD_DB}"
        MYSQL_PASSWORD: "${SERVICE_PASSWORD_DB}"   # same generated value
```

Both end up writing the generated secret to `.env`. The declared form lets you
tune `length`/`special_chars`; the magic form is terser. (This is shown for
illustration only — templates are **not** mass-converted; existing templates
keep working unchanged.)

---

## 4. Files

`files:` materializes config files next to the compose project and bind-mounts
them into the container. `${VAR}` and `${SERVICE_*}` tokens in `content` are
substituted.

```yaml
files:
  - path: /etc/app/config.yaml      # path inside the container
    mode: 0o644
    content: |
      secret: ${SERVICE_BASE64_CONFIG}
      user: ${SERVICE_USER_APP}
```

The installer writes the file locally (basename of `path`) and replaces the
matching named volume / directory mount with a bind mount. Files are re-rendered
on update so config survives a compose regeneration.

---

## 5. Validation

Two validators are available:

- `TemplateService.validate_template(template)` — the loader's check: required
  fields, `compose`/`dockerfile` presence, well-formed `variables`. Returns
  `{'valid': bool, 'errors': [...]}`.
- `TemplateService.validate_catalog_entry(entry)` — a lightweight catalog-level
  check that wraps the above and additionally verifies the `id` slug, flags
  unknown variable `type` values, and flags malformed magic tokens. Returns
  `{'valid': bool, 'errors': [...], 'warnings': [...]}`. Warnings are
  non-fatal so the loader stays permissive.

---

## 6. API

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/templates/` | List templates (local + remote). |
| `GET` | `/api/v1/templates/categories` | List categories. |
| `GET` | `/api/v1/templates/<id>` | Template detail (processed variables). |
| `GET` | `/api/v1/templates/catalog/schema` | Machine-readable description of variable types + magic tokens (this document, in JSON). |
| `POST` | `/api/v1/templates/<id>/install` | Install as a new app (admin). |
| `POST` | `/api/v1/templates/validate-install` | Pre-flight validation. |
| `POST` | `/api/v1/templates/sync` | Fetch templates from configured repos (admin). |
| `GET`/`POST`/`DELETE` | `/api/v1/templates/repos` | Manage repositories. |
| `GET` | `/api/v1/templates/repos/index` | The publishable `index.json` for this instance. |

### `GET /catalog/schema` response (shape)

```json
{
  "schema_version": "1.0",
  "variable_types": [
    {"type": "string",   "auto_generated": false, "description": "..."},
    {"type": "password", "auto_generated": true,  "description": "..."},
    {"type": "port",     "auto_generated": true,  "description": "..."},
    {"type": "uuid",     "auto_generated": true,  "description": "..."},
    {"type": "random",   "auto_generated": true,  "description": "..."}
  ],
  "magic_variables": [
    {"token": "${SERVICE_PASSWORD_<NAME>}", "description": "..."},
    {"token": "${SERVICE_USER_<NAME>}",     "description": "..."},
    {"token": "${SERVICE_FQDN_<NAME>}",     "description": "..."},
    {"token": "${SERVICE_URL_<NAME>}",      "description": "..."},
    {"token": "${SERVICE_BASE64_<NAME>}",   "description": "..."}
  ],
  "notes": ["..."]
}
```
