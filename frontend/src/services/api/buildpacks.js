// Build Packs API — transparent, zero-Dockerfile build planning.
// Backend blueprint mounted at /api/v1/buildpacks.

// Detect the stack for a repository and return a build plan + generated
// Dockerfile/compose preview. Body: { repo_url?, branch?, source_connection_id?,
// repository_full_name?, path?, name? }.
export async function detectBuildpack(body) {
    return this.request('/buildpacks/detect', {
        method: 'POST',
        body,
    });
}

// Generate a Dockerfile + compose from a plan (pure, no clone).
// Body: { plan, overrides?, name? }.
export async function generateBuildpack(plan, overrides, name) {
    return this.request('/buildpacks/generate', {
        method: 'POST',
        body: { plan, overrides, name },
    });
}
