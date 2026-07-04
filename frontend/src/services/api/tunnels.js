// Remote Access — WireGuard tunnels + the services published over them.
// Backend: backend/app/api/tunnels.py. See docs/REMOTE_ACCESS_ROADMAP.md.

export async function getTunnels() {
    return this.request('/tunnels/');
}

export async function getTunnel(id, { refresh = true } = {}) {
    return this.request(`/tunnels/${id}${refresh ? '' : '?refresh=0'}`);
}

export async function createTunnel(data) {
    // data: { edge_server_id, private_server_id, name? }
    return this.request('/tunnels/', { method: 'POST', body: data });
}

export async function deleteTunnel(id) {
    return this.request(`/tunnels/${id}`, { method: 'DELETE' });
}

export async function getTunnelServices(tunnelId) {
    return this.request(`/tunnels/${tunnelId}/services`);
}

export async function publishTunnelService(tunnelId, data) {
    // data: { hostname, port, require_auth?, auth_username?, auth_password?, ssl? }
    return this.request(`/tunnels/${tunnelId}/services`, { method: 'POST', body: data });
}

export async function unpublishTunnelService(tunnelId, serviceId) {
    return this.request(`/tunnels/${tunnelId}/services/${serviceId}`, { method: 'DELETE' });
}
