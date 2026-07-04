// Deployment jobs and logs

export async function getDeploymentJobs(params = {}) {
    const query = new URLSearchParams();
    if (params.status) query.append('status', params.status);
    if (params.serverId) query.append('server_id', params.serverId);
    if (params.limit) query.append('limit', params.limit);
    const suffix = query.toString() ? `?${query}` : '';
    return this.request(`/deployment-jobs${suffix}`);
}

export async function getDeploymentJob(jobId, includeLogs = true) {
    return this.request(`/deployment-jobs/${jobId}?logs=${includeLogs}`);
}

export async function getDeploymentJobLogs(jobId, afterId = null) {
    const suffix = afterId ? `?after_id=${afterId}` : '';
    return this.request(`/deployment-jobs/${jobId}/logs${suffix}`);
}
