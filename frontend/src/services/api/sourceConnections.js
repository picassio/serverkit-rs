// Source provider connections

export async function getGithubSourceStatus() {
    return this.request('/source-connections/github/status');
}

export async function startSourceConnection(provider, redirectUri) {
    return this.request(`/source-connections/${provider}/authorize?redirect_uri=${encodeURIComponent(redirectUri)}`);
}

export async function completeSourceConnection(provider, code, state, redirectUri) {
    return this.request(`/source-connections/${provider}/callback`, {
        method: 'POST',
        body: { code, state, redirect_uri: redirectUri },
    });
}

export async function disconnectSourceConnection(provider) {
    return this.request(`/source-connections/${provider}`, { method: 'DELETE' });
}

export async function listGithubRepositories({ search = '', page = 1, perPage = 50 } = {}) {
    const params = new URLSearchParams();
    if (search) params.set('search', search);
    params.set('page', page);
    params.set('per_page', perPage);
    return this.request(`/source-connections/github/repos?${params.toString()}`);
}

export async function listGithubBranches(fullName) {
    return this.request(`/source-connections/github/repos/${encodeURIComponent(fullName)}/branches`);
}

export async function inspectGithubRepositoryManifest(fullName, ref = null) {
    const params = new URLSearchParams();
    if (ref) params.set('ref', ref);
    const query = params.toString();
    return this.request(`/source-connections/github/repos/${encodeURIComponent(fullName)}/manifest${query ? `?${query}` : ''}`);
}

export async function getGithubSourceConfig() {
    return this.request('/source-connections/admin/github');
}

export async function updateGithubSourceConfig(config) {
    return this.request('/source-connections/admin/github', {
        method: 'PUT',
        body: config,
    });
}

// GitLab mirrors the GitHub surface (same generic connect/disconnect helpers
// above, which already take a `provider` argument).

export async function getGitlabSourceStatus() {
    return this.request('/source-connections/gitlab/status');
}

export async function listGitlabRepositories({ search = '', page = 1, perPage = 50 } = {}) {
    const params = new URLSearchParams();
    if (search) params.set('search', search);
    params.set('page', page);
    params.set('per_page', perPage);
    return this.request(`/source-connections/gitlab/repos?${params.toString()}`);
}

export async function listGitlabBranches(fullName) {
    return this.request(`/source-connections/gitlab/repos/${encodeURIComponent(fullName)}/branches`);
}

export async function inspectGitlabRepositoryManifest(fullName, ref = null) {
    const params = new URLSearchParams();
    if (ref) params.set('ref', ref);
    const query = params.toString();
    return this.request(`/source-connections/gitlab/repos/${encodeURIComponent(fullName)}/manifest${query ? `?${query}` : ''}`);
}

export async function getGitlabSourceConfig() {
    return this.request('/source-connections/admin/gitlab');
}

export async function updateGitlabSourceConfig(config) {
    return this.request('/source-connections/admin/gitlab', {
        method: 'PUT',
        body: config,
    });
}

// Bitbucket mirrors the GitHub surface.

export async function getBitbucketSourceStatus() {
    return this.request('/source-connections/bitbucket/status');
}

export async function listBitbucketRepositories({ search = '', page = 1, perPage = 50 } = {}) {
    const params = new URLSearchParams();
    if (search) params.set('search', search);
    params.set('page', page);
    params.set('per_page', perPage);
    return this.request(`/source-connections/bitbucket/repos?${params.toString()}`);
}

export async function listBitbucketBranches(fullName) {
    return this.request(`/source-connections/bitbucket/repos/${encodeURIComponent(fullName)}/branches`);
}

export async function inspectBitbucketRepositoryManifest(fullName, ref = null) {
    const params = new URLSearchParams();
    if (ref) params.set('ref', ref);
    const query = params.toString();
    return this.request(`/source-connections/bitbucket/repos/${encodeURIComponent(fullName)}/manifest${query ? `?${query}` : ''}`);
}

export async function getBitbucketSourceConfig() {
    return this.request('/source-connections/admin/bitbucket');
}

export async function updateBitbucketSourceConfig(config) {
    return this.request('/source-connections/admin/bitbucket', {
        method: 'PUT',
        body: config,
    });
}
