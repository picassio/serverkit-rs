import { useState } from 'react';
import api from '../../services/api';
import { useToast } from '../../contexts/ToastContext';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

// Surfaces everything the agent's capability probe reports: detected
// runtimes (python/node/php/go/ruby/java versions), runtime version
// managers (pyenv/pyenv-win), package manager + sudo mode + systemd
// JSON support, allowed file roots, and the boolean capability map.
//
// Reads from the server payload — when the agent is offline, the
// fields fall back to the most recent snapshot persisted to the DB by
// agent_registry.update_capabilities. capabilities_stale flips true in
// that case and we badge the card accordingly.
export default function SystemStatusCard({ server, onRefresh }) {
    const toast = useToast();
    const [refreshing, setRefreshing] = useState(false);

    const stale = !!server.capabilities_stale;
    const probedAt = server.capabilities_at;
    const platform = server.platform || server.os_type;
    const distro = server.distro || server.os_version;
    const sudo = server.sudo;
    const caps = server.capabilities || {};
    const runtimes = server.runtimes || {};
    const managers = server.runtime_managers || {};
    const allowedPaths = server.allowed_paths || [];

    async function handleRefresh() {
        setRefreshing(true);
        try {
            await api.refreshRemoteCapabilities(server.id);
            toast.success('Capabilities re-probed');
            if (onRefresh) await onRefresh();
        } catch (err) {
            toast.error(err.message || 'Refresh failed');
        } finally {
            setRefreshing(false);
        }
    }

    const capabilityKeys = Object.keys(caps).sort();
    const runtimeKeys = Object.keys(runtimes).filter((k) => runtimes[k]).sort();

    return (
        <div className="info-card system-status-card">
            <div className="system-status-card__header">
                <div className="system-status-card__title">
                    <h3>System Status</h3>
                    {stale && (
                        <Badge variant="outline" title="Agent offline — showing last cached snapshot">
                            Stale
                        </Badge>
                    )}
                </div>
                <div className="system-status-card__header-meta">
                    {probedAt && (
                        <span className="system-status-card__probed-at">
                            probed {formatRelative(probedAt)}
                        </span>
                    )}
                    <Button
                        size="sm"
                        variant="outline"
                        onClick={handleRefresh}
                        disabled={refreshing || !server.is_connected}
                        title={server.is_connected ? 'Re-run capability probe on the agent' : 'Agent must be online to refresh'}
                    >
                        {refreshing ? 'Refreshing…' : 'Refresh'}
                    </Button>
                </div>
            </div>

            <div className="system-status-card__rows">
                <Row label="Platform" value={[platform, distro].filter(Boolean).join(' · ') || 'Unknown'} />
                <Row label="Sudo" value={<SudoBadge mode={sudo} />} />
                <Row
                    label="Package manager"
                    value={detectedPackageManager(caps, server) || <Muted>not detected</Muted>}
                />
                <Row
                    label="Runtime managers"
                    value={
                        Object.keys(managers).length === 0 ? (
                            <Muted>none</Muted>
                        ) : (
                            <span className="system-status-card__chips">
                                {Object.entries(managers).map(([rt, mgr]) =>
                                    mgr ? (
                                        <Badge key={rt} variant="secondary">
                                            {rt}: {mgr}
                                        </Badge>
                                    ) : (
                                        <Badge key={rt} variant="outline" title={`No ${rt} manager installed`}>
                                            {rt}: none
                                        </Badge>
                                    )
                                )}
                            </span>
                        )
                    }
                />
                <Row
                    label="Detected runtimes"
                    value={
                        runtimeKeys.length === 0 ? (
                            <Muted>none reported</Muted>
                        ) : (
                            <span className="system-status-card__chips">
                                {runtimeKeys.map((rt) => (
                                    <Badge key={rt} variant="outline">
                                        {rt} {runtimes[rt] || '?'}
                                    </Badge>
                                ))}
                            </span>
                        )
                    }
                />
                <Row
                    label="Capabilities"
                    value={
                        capabilityKeys.length === 0 ? (
                            <Muted>none reported (older agent?)</Muted>
                        ) : (
                            <span className="system-status-card__chips">
                                {capabilityKeys.map((k) => (
                                    <Badge
                                        key={k}
                                        variant={caps[k] ? 'default' : 'outline'}
                                        title={caps[k] ? 'available' : 'not available on this host'}
                                    >
                                        {k}
                                    </Badge>
                                ))}
                            </span>
                        )
                    }
                />
                {allowedPaths.length > 0 && (
                    <Row
                        label="File access roots"
                        value={
                            <ul className="system-status-card__paths">
                                {allowedPaths.map((p) => (
                                    <li key={p} className="mono">{p}</li>
                                ))}
                            </ul>
                        }
                    />
                )}
            </div>
        </div>
    );
}

function Row({ label, value }) {
    return (
        <div className="system-status-card__row">
            <span className="system-status-card__row-label">{label}</span>
            <span className="system-status-card__row-value">{value}</span>
        </div>
    );
}

function Muted({ children }) {
    return <span className="text-muted-foreground">{children}</span>;
}

function SudoBadge({ mode }) {
    if (mode === 'root') return <Badge variant="default" title="Agent is running as root">root</Badge>;
    if (mode === 'passwordless') return <Badge variant="default" title="sudo -n succeeded">passwordless sudo</Badge>;
    if (mode === 'unavailable') {
        return (
            <Badge variant="destructive" title="Privileged actions (apt, systemd) will fail">
                unavailable
            </Badge>
        );
    }
    return <Muted>not reported</Muted>;
}

// We don't have a dedicated package manager field, but the capability
// probe sets caps.packages true when *some* manager is on PATH. Surface
// the distro hint so users know what'll be invoked.
function detectedPackageManager(caps, server) {
    if (!caps.packages) return null;
    const distro = (server.distro || server.os_type || '').toLowerCase();
    if (distro.includes('ubuntu') || distro.includes('debian')) return 'apt';
    if (distro.includes('rhel') || distro.includes('fedora') || distro.includes('centos') || distro.includes('rocky')) return 'dnf';
    if (distro.includes('alpine')) return 'apk';
    if (distro.includes('arch')) return 'pacman';
    if (distro.includes('suse') || distro.includes('opensuse')) return 'zypper';
    return 'available';
}

function formatRelative(iso) {
    if (!iso) return '';
    const ts = new Date(iso).getTime();
    if (!Number.isFinite(ts)) return iso;
    const diff = Date.now() - ts;
    if (diff < 60_000) return 'just now';
    if (diff < 3600_000) return `${Math.round(diff / 60_000)}m ago`;
    if (diff < 86400_000) return `${Math.round(diff / 3600_000)}h ago`;
    return new Date(iso).toLocaleString();
}
