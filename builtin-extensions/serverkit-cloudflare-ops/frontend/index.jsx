// Cloudflare Zone Ops UI, contributed through the extension system. The zone-ops
// backend (the /api/v1/cloudflare blueprint + CloudflareService) lives in this
// extension's backend/; the page + panels + api client + SCSS stay in the host
// tree and are re-used here (re-export pattern, same as serverkit-git/email/
// wordpress) so their many relative imports keep resolving. DNS records and the
// Cloudflare connection itself stay core (they back /domains) — this page only
// adds the per-zone control panel, reached from the "Open in Cloudflare" button
// on a Cloudflare-managed domain.
//
// After sync this file lives at frontend/src/plugins/serverkit-cloudflare-ops/ so
// the relative import resolves against the host's pages directory.
//
// It's a single plain full-bleed route (cloudflare/zones/:zoneId, own PageTopbar
// + internal Settings/WAF/Workers/Tunnels/Storage tabs), so no tab-group shell or
// self-rendered sub-router is needed — unlike WordPress.
import CloudflareZoneSettings from '../../pages/CloudflareZoneSettings';

export function CloudflareZoneSettingsPage() {
    return <CloudflareZoneSettings />;
}

// No default export on purpose: PluginLoader legacy-auto-renders any plugin
// default export globally (the page then runs with zoneId=undefined). The
// route contribution resolves the NAMED export via resolveComponent.
