// Managed reverse-proxy stack (opt-in Traefik/Caddy as a Compose stack).
// Host nginx stays the default. Mounted by the backend at /api/v1/servers,
// so paths are /servers/<id>/proxy*.

export async function getServerProxy(serverId) {
    return this.request(`/servers/${serverId}/proxy`);
}

// Ingress-plane audit for a single server: compares each app's expected ingress
// plane against the server's active proxy. Returns
// { server_id, server_name, proxy_type, expected_plane, app_count,
//   mismatch_count, apps: [{ id, name, app_type, ingress_plane, mismatch, reason }] }.
export async function getServerIngressAudit(serverId) {
    return this.request(`/servers/${serverId}/proxy/ingress-audit`);
}

// Fleet-wide proxy posture: one row per server. Returns { servers: [...] }.
// Backed by GET /api/v1/servers/proxy/overview (a static prefix that never
// collides with the per-server /servers/<id>/proxy route).
export async function getFleetProxyOverview() {
    return this.request('/servers/proxy/overview');
}

export async function getServerProxyComposePreview(serverId, options = {}) {
    const params = new URLSearchParams();
    if (options.proxyType) params.set('proxy_type', options.proxyType);
    if (options.acmeEmail) params.set('acme_email', options.acmeEmail);
    if (options.dashboard) params.set('dashboard', '1');
    const qs = params.toString();
    return this.request(`/servers/${serverId}/proxy/compose-preview${qs ? `?${qs}` : ''}`);
}

export async function configureServerProxy(serverId, data) {
    return this.request(`/servers/${serverId}/proxy/configure`, {
        method: 'POST',
        body: data,
    });
}

export async function regenerateServerProxy(serverId, data = {}) {
    return this.request(`/servers/${serverId}/proxy/regenerate`, {
        method: 'POST',
        body: data,
    });
}

export async function switchServerProxy(serverId, proxyType) {
    return this.request(`/servers/${serverId}/proxy/switch`, {
        method: 'POST',
        body: { proxy_type: proxyType },
    });
}
