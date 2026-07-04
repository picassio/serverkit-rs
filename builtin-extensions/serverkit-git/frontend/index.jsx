// Re-exports the existing host Git page through the extension system.
// The extension owns the canonical /git route; the host keeps the page
// component as implementation detail until the Git UI is fully split.
//
// After install, this file lives at frontend/src/plugins/serverkit-git/
// so the relative import resolves against the host's pages directory.
import GitPage from '../../pages/Git';

export function GitExtensionPage() {
    return <GitPage basePath="/git" />;
}

// Backward compatibility for older installed manifests that still point
// at GitExtPage. Contribution normalization rewrites those manifests, but
// keeping the export avoids a blank route if stale metadata slips through.
export const GitExtPage = GitExtensionPage;
