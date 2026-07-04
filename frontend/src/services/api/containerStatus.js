// Centralized container status aggregator API.
// Backend blueprint mounted at /api/v1/status.

export async function getAppContainerStatus(appId) {
    return this.request(`/status/app/${appId}`);
}

export async function getAppsContainerStatus() {
    return this.request('/status/apps');
}
