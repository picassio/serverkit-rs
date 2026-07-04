# ADR 0001 — Production frontend delivery for extensions

**Status:** Accepted (2026-07-01) · Task #22 of the Extensions Platform plan
**Context:** the §1.3 constraint — the load-bearing blocker for third-party UI.

## Problem

`frontend/src/plugins/contributions.js` discovers plugin UI at **build time** via
`import.meta.glob('../plugins/*/index.{js,jsx}')`. A production panel ships a
**prebuilt** Vite bundle (Docker image / `frontend/dist`). Installing a plugin
copies files onto disk but nothing rebuilds the bundle, so:

- **Builtin (in-repo) extensions** render in production, because their frontend is
  checked into `frontend/src/plugins/<slug>/` and compiled into every build.
- **Third-party extensions that ship a frontend do NOT render** on a prebuilt panel.
  Their **backend halves load fine** (blueprints are hot-loaded at runtime).

## Options considered

**(a) Per-plugin prebuilt ESM bundle.** Plugin repos ship a built `dist/` ESM
module; the panel serves it and `import()`s it at runtime, with React and shared
libs provided as externals via an import map. *Pro:* real third-party UI with no
panel rebuild. *Con:* version-skew risk (React/singleton duplication), a public
module contract to keep stable, build tooling every plugin author must adopt, and
a meaningfully larger runtime loader. High effort, real fragility.

**(b) Iframe / `bare`-layout escape hatch.** A plugin renders a fullscreen page
the panel embeds. *Pro:* total isolation, no shared-dep problem. *Con:* not a
first-class contribution (no nav integration, no shared theme/components), clunky.
Fine as an escape hatch, not the default.

**(c) Backend-only third parties + builtin-only frontends (the current reality).**
Third-party extensions ship backend blueprints (which work everywhere today);
any first-class UI ships as an in-repo builtin, pre-bundled per D5. *Pro:* zero new
machinery, zero version-skew risk, already proven (`serverkit-git`, `serverkit-gui`).
*Con:* a third party can't ship arbitrary bundled UI without a panel release.

## Decision

**Ship (c) as the supported posture now; keep (b) as an escape hatch; treat (a) as
the documented future direction, not yet built.** This matches the plan's own risk
mitigation (§Risks) and decision D5, and unblocks Phases 4–5 (extraction of *core*
verticals into *builtin* extensions), which never needed (a) in the first place.

Concretely, what we guarantee today:

1. **Builtin frontends are the first-class UI path** — pre-bundled, single source of
   truth enforced by `scripts/sync-builtin-frontends.mjs` + the Extensions CI drift
   gate. Every extracted core vertical (Email, WordPress, Tier-1 pages) uses this.
2. **Third-party backend extensions are fully supported** — hot-loaded blueprints,
   status guard, permissions, data models, jobs, sockets (Phase 3 primitives).
3. **Graceful degradation, never a white-screen** — when a contributed `component`
   or custom `layout` can't be resolved from the compiled bundle, the route/widget
   is **skipped and warned** (already implemented in `ExtensionRoutes.jsx` /
   `contributions.js` / `PluginLoader.jsx`), so a plugin whose frontend isn't in the
   bundle degrades to "backend works, UI absent" rather than crashing the SPA.
4. **A plugin-asset serving route exists** (`GET /api/v1/plugins/<slug>/assets/...`)
   as the foundation for (a): a panel can serve a plugin's static/prebuilt files.
   The dynamic-`import()` loader + import-map externals are deliberately **out of
   scope** until a third party needs bundled UI badly enough to justify the skew risk.

## Consequences

- Phases 4–5 proceed now (builtin extraction is unaffected by this blocker).
- The registry's near-term third-party entries are **backend-only or builtin**
  (e.g. `serverkit-gui` is backend + a widget whose code is pre-bundled).
- Revisiting (a) is a self-contained follow-up: implement the loader + externals on
  top of the already-shipped asset-serving route and graceful-skip guarantee.
