// Cloud Provisioning, contributed through the extension system. The page
// component and its /api/v1/cloud-provisioning backend stay in core for now
// (two-speed extraction, D2); this extension owns the Cloud Servers tab in the
// core Servers group via a tab-group contribution (#43) plus the route/palette
// entries in its manifest.
//
// After sync this file lives at frontend/src/plugins/serverkit-cloud-provision/
// so the relative import resolves against the host's pages directory.
import CloudProvision from '../../pages/CloudProvision';

export function CloudProvisionPage() {
    return <CloudProvision />;
}
