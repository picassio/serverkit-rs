// PR Preview Environments — per-app preview list, settings, and lifecycle ops.
// Mounted under /apps on the backend.

export async function getPreviews(appId) {
    return this.request(`/apps/${appId}/previews`);
}

export async function getPreviewSettings(appId) {
    return this.request(`/apps/${appId}/previews/settings`);
}

export async function updatePreviewSettings(appId, body) {
    return this.request(`/apps/${appId}/previews/settings`, { method: 'PUT', body });
}

export async function syncPreviews(appId) {
    return this.request(`/apps/${appId}/previews/sync`, { method: 'POST' });
}

export async function redeployPreview(appId, previewId) {
    return this.request(`/apps/${appId}/previews/${previewId}/redeploy`, { method: 'POST' });
}

export async function destroyPreview(appId, previewId) {
    return this.request(`/apps/${appId}/previews/${previewId}`, { method: 'DELETE' });
}
