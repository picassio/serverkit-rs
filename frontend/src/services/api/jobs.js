// Unified job system API (admin only) — backs the Jobs page.

export async function getJobs(params = {}) {
    const query = new URLSearchParams();
    if (params.status) query.append('status', params.status);
    if (params.kind) query.append('kind', params.kind);
    if (params.owner_type) query.append('owner_type', params.owner_type);
    if (params.owner_id) query.append('owner_id', params.owner_id);
    if (params.limit) query.append('limit', params.limit);
    if (params.offset) query.append('offset', params.offset);
    const suffix = query.toString() ? `?${query}` : '';
    return this.request(`/jobs${suffix}`);
}

export async function getJobStats() {
    return this.request('/jobs/stats');
}

export async function getJobKinds() {
    return this.request('/jobs/kinds');
}

export async function getJob(jobId) {
    return this.request(`/jobs/${jobId}`);
}

export async function cancelJob(jobId) {
    return this.request(`/jobs/${jobId}/cancel`, { method: 'POST' });
}

export async function retryJob(jobId) {
    return this.request(`/jobs/${jobId}/retry`, { method: 'POST' });
}

export async function getScheduledJobs() {
    return this.request('/jobs/scheduled');
}

export async function runScheduledJob(scheduledId) {
    return this.request(`/jobs/scheduled/${scheduledId}/run`, { method: 'POST' });
}

export async function setScheduledJobEnabled(scheduledId, enabled) {
    return this.request(`/jobs/scheduled/${scheduledId}/enabled`, {
        method: 'POST',
        body: JSON.stringify({ enabled }),
    });
}
