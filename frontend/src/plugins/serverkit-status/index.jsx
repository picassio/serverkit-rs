// Status Pages management, contributed through the extension system. The page
// component and its /api/v1/status-pages + /api/v1/uptime backends stay in core
// for now (two-speed extraction, D2), and the PUBLIC /status/:slug page stays a
// core route (unauthenticated — the contribution system only places routes
// behind auth). This extension owns the admin tab in the core Observability
// group via a tab-group contribution (#43) plus the route/palette entries in
// its manifest.
//
// After sync this file lives at frontend/src/plugins/serverkit-status/ so the
// relative import resolves against the host's pages directory.
import StatusPages from '../../pages/StatusPages';

export function StatusPagesPage() {
    return <StatusPages />;
}
