import { X, AlertTriangle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

export function UsageCell({ percent = 0, variant = 'cpu' }) {
    const clamped = Math.min(Number(percent) || 0, 100);
    return (
        <div className="usage-cell">
            <div className={`usage-bar ${variant}`} style={{ width: `${clamped}%` }} />
            <span>{clamped.toFixed(1)}%</span>
        </div>
    );
}

export function ProcessTable({
    processes = [],
    selectedPid = null,
    onSelect,
    onKill,
    onForceKill,
    formatMemory = defaultFormatMemory,
    getStatusVariant = defaultGetStatusVariant,
}) {
    return (
        <div className="processes-table-wrapper">
            <table
                className="table processes-table"
                spellCheck={false}
                data-gramm="false"
                data-gramm_editor="false"
                data-enable-grammarly="false"
            >
                <thead>
                    <tr>
                        <th>PID</th>
                        <th>Name</th>
                        <th>User</th>
                        <th>CPU %</th>
                        <th>Memory %</th>
                        <th>Memory</th>
                        <th>Status</th>
                        <th>Actions</th>
                    </tr>
                </thead>
                <tbody>
                    {processes.map(p => (
                        <tr
                            key={p.pid}
                            className={selectedPid === p.pid ? 'selected' : ''}
                            onClick={() => onSelect?.(p)}
                        >
                            <td className="mono">{p.pid}</td>
                            <td><div className="process-name"><span>{p.name}</span></div></td>
                            <td>{p.user}</td>
                            <td><UsageCell percent={p.cpu_percent} variant="cpu" /></td>
                            <td><UsageCell percent={p.memory_percent} variant="memory" /></td>
                            <td>{formatMemory(p.memory_info?.rss)}</td>
                            <td>
                                <Badge variant={getStatusVariant(p.status)}>{p.status}</Badge>
                            </td>
                            <td>
                                <div className="action-buttons">
                                     {onKill && (
                                        <Button
                                            variant="outline"
                                            size="icon"
                                            className="process-action-button"
                                            onClick={(e) => { e.stopPropagation(); onKill(p); }}
                                            title="Kill"
                                            aria-label={`Kill ${p.name}`}
                                        >
                                            <X size={12} />
                                        </Button>
                                    )}
                                    {onForceKill && (
                                        <Button
                                            variant="destructive"
                                            size="icon"
                                            className="process-action-button"
                                            onClick={(e) => { e.stopPropagation(); onForceKill(p); }}
                                            title="Force Kill"
                                            aria-label={`Force kill ${p.name}`}
                                        >
                                            <AlertTriangle size={12} />
                                        </Button>
                                    )}
                                </div>
                            </td>
                        </tr>
                    ))}
                </tbody>
            </table>
        </div>
    );
}

export function ProcessDetailsPanel({ process, onClose, formatMemory = defaultFormatMemory }) {
    if (!process) return null;
    return (
        <div className="process-details-panel">
            <div className="panel-header">
                <h3>Process Details</h3>
                <Button variant="outline" size="sm" onClick={onClose}>Close</Button>
            </div>
            <div className="panel-body">
                <div className="details-grid">
                    <DetailItem label="PID" value={process.pid} mono />
                    <DetailItem label="Name" value={process.name} />
                    <DetailItem label="User" value={process.user} />
                    <DetailItem label="Status" value={process.status} />
                    <DetailItem label="CPU" value={`${(process.cpu_percent || 0).toFixed(2)}%`} />
                    <DetailItem label="Memory" value={formatMemory(process.memory_info?.rss)} />
                    <DetailItem label="Threads" value={process.num_threads} />
                    <DetailItem
                        label="Created"
                        value={process.create_time ? new Date(process.create_time * 1000).toLocaleString() : '-'}
                    />
                </div>
                {process.command && (
                    <div className="command-line">
                        <span className="detail-label">Command</span>
                        <code>{process.command}</code>
                    </div>
                )}
            </div>
        </div>
    );
}

export function DetailItem({ label, value, mono = false, children }) {
    const valueClass = ['detail-value', mono && 'mono'].filter(Boolean).join(' ');
    return (
        <div className="detail-item">
            <span className="detail-label">{label}</span>
            {children ?? <span className={valueClass}>{value}</span>}
        </div>
    );
}

function defaultFormatMemory(bytes) {
    if (!bytes) return '-';
    const units = ['B', 'KB', 'MB', 'GB'];
    let i = 0;
    while (bytes >= 1024 && i < units.length - 1) {
        bytes /= 1024;
        i++;
    }
    return `${bytes.toFixed(1)} ${units[i]}`;
}

function defaultGetStatusVariant(status) {
    switch (status?.toLowerCase()) {
        case 'running':
        case 'sleeping':
            return 'success';
        case 'stopped':
        case 'zombie':
            return 'destructive';
        case 'idle':
        case 'disk-sleep':
            return 'warning';
        default:
            return 'secondary';
    }
}

export default ProcessTable;
