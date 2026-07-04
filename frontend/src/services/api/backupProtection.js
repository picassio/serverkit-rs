// Backup "Protection" policy + runs API.
//
// Generic over the target so the shared ProtectionPanel can drive both
// WordPress sites and applications with one set of methods. The backend mounts
// the same endpoint shapes under /wordpress/sites/:id and /apps/:id.

function policyBase(targetType, targetId) {
    return targetType === 'wordpress_site'
        ? `/wordpress/sites/${targetId}/backup-policy`
        : `/apps/${targetId}/backup-policy`;
}

function runsBase(targetType, targetId) {
    return targetType === 'wordpress_site'
        ? `/wordpress/sites/${targetId}/backups`
        : `/apps/${targetId}/backups`;
}

export async function getBackupPolicy(targetType, targetId) {
    return this.request(policyBase(targetType, targetId));
}

export async function updateBackupPolicy(targetType, targetId, policy) {
    return this.request(policyBase(targetType, targetId), { method: 'PUT', body: policy });
}

export async function triggerBackup(targetType, targetId) {
    return this.request(runsBase(targetType, targetId), { method: 'POST' });
}

export async function getBackupRuns(targetType, targetId) {
    return this.request(runsBase(targetType, targetId));
}

export async function restoreBackupRun(targetType, targetId, runId, options) {
    return this.request(`${runsBase(targetType, targetId)}/${runId}/restore`, {
        method: 'POST',
        body: options || {},
    });
}

export async function verifyBackupRun(targetType, targetId, runId) {
    return this.request(`${runsBase(targetType, targetId)}/${runId}/verify`, { method: 'POST' });
}

export async function deleteBackupRun(targetType, targetId, runId) {
    return this.request(`${runsBase(targetType, targetId)}/${runId}`, { method: 'DELETE' });
}
