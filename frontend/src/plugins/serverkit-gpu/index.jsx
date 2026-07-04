// GPU Monitor, contributed through the extension system. The page component and
// its /api/v1/gpu backend stay in core for now (two-speed extraction, D2); this
// extension owns the /gpu route + sidebar/palette entries via its manifest.
//
// After sync this file lives at frontend/src/plugins/serverkit-gpu/ so the
// relative import resolves against the host's pages directory.
import GpuMonitor from '../../pages/GpuMonitor';

export function GpuMonitorPage() {
    return <GpuMonitor />;
}
