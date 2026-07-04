// Deployment config snapshots + diff/restore.
// Mounted by the backend at /api/v1/apps.

export async function getAppSnapshots(appId, limit = 50) {
    return this.request(`/apps/${appId}/snapshots?limit=${limit}`);
}

export async function getAppSnapshot(appId, snapId) {
    return this.request(`/apps/${appId}/snapshots/${snapId}`);
}

export async function getSnapshotDiff(appId, snapId, against = 'previous') {
    const query = against ? `?against=${encodeURIComponent(against)}` : '';
    return this.request(`/apps/${appId}/snapshots/${snapId}/diff${query}`);
}

export async function restoreSnapshot(appId, snapId) {
    return this.request(`/apps/${appId}/snapshots/${snapId}/restore`, {
        method: 'POST',
    });
}

// "Config Checkpoint" is the user-facing name for a deployment config snapshot
// (§8). These aliases let new UI code use the new term; the backend serves both
// /snapshots and /config-checkpoints.
export async function getConfigCheckpoints(appId, limit = 50) {
    return getAppSnapshots.call(this, appId, limit);
}

export async function getConfigCheckpoint(appId, id) {
    return getAppSnapshot.call(this, appId, id);
}

export async function getConfigCheckpointDiff(appId, id, against = 'previous') {
    return getSnapshotDiff.call(this, appId, id, against);
}

export async function restoreConfigCheckpoint(appId, id) {
    return restoreSnapshot.call(this, appId, id);
}
