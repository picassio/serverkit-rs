import { Pill } from '@/components/ds';
import { Button } from '@/components/ui/button';
import EmptyState from '@/components/EmptyState';
import { Archive, RotateCcw, ShieldCheck, Trash2, HardDrive, Cloud, Layers } from 'lucide-react';
import { humanSize, formatMoney, formatWhen, statusKind, storageLabel } from './format';

// Card 3 of the backup "Protection" panel: a data-table of backup runs.
// Pairs with the .sk-dtable styles plus a .backup-history-list-scoped layer.
function storageIcon(run) {
    const label = storageLabel(run);
    if (label === 'both') {
        return (
            <span className="backup-history-list__storage" title="Local + remote">
                <Layers size={13} /> both
            </span>
        );
    }
    if (label === 'remote') {
        return (
            <span className="backup-history-list__storage" title="Remote">
                <Cloud size={13} /> remote
            </span>
        );
    }
    return (
        <span className="backup-history-list__storage" title="Local">
            <HardDrive size={13} /> local
        </span>
    );
}

export default function BackupHistoryList({
    runs,
    loading,
    onRestore,
    onVerify,
    onDelete,
    onRowClick,
}) {
    if (loading && (!runs || runs.length === 0)) {
        return <EmptyState icon={Archive} title="Loading backups…" loading />;
    }

    if (!loading && (!runs || runs.length === 0)) {
        return (
            <EmptyState
                icon={Archive}
                title="No backups yet"
                description="Turn on protection or click Back up now."
            />
        );
    }

    return (
        <table className="sk-dtable backup-history-list">
            <thead>
                <tr>
                    <th>Backup</th>
                    <th>Date</th>
                    <th>Size</th>
                    <th>Cost</th>
                    <th>Status</th>
                    <th>Storage</th>
                    <th aria-label="Actions" />
                </tr>
            </thead>
            <tbody>
                {runs.map((run) => (
                    <tr
                        key={run.id}
                        className={`backup-history-list__row ${onRowClick ? 'is-clickable' : ''}`}
                        onClick={onRowClick ? () => onRowClick(run) : undefined}
                    >
                        <td>
                            <div className="sk-cell-name">
                                <span className="backup-history-list__ico"><Archive size={14} /></span>
                                <span>{run.metadata?.backup_name || `Backup #${run.id}`}</span>
                                <Pill kind={run.kind === 'full' ? 'violet' : 'gray'} dot={false}>{run.kind}</Pill>
                            </div>
                        </td>
                        <td className="backup-history-list__when">{formatWhen(run.started_at)}</td>
                        <td className="sk-cell-mono">{humanSize(run.size_total)}</td>
                        <td className="sk-cell-mono">{formatMoney(run.cost_total)}</td>
                        <td><Pill kind={statusKind(run.status)}>{run.status}</Pill></td>
                        <td>{storageIcon(run)}</td>
                        <td>
                            <div className="backup-history-list__actions" onClick={(e) => e.stopPropagation()}>
                                <Button size="icon" variant="outline" title="Restore" disabled={run.status !== 'success'} onClick={() => onRestore(run)}><RotateCcw size={14} /></Button>
                                {run.remote_key && (
                                    <Button size="icon" variant="outline" title="Verify remote copy" onClick={() => onVerify(run)}><ShieldCheck size={14} /></Button>
                                )}
                                <Button size="icon" variant="destructive" title="Delete" onClick={() => onDelete(run)}><Trash2 size={14} /></Button>
                            </div>
                        </td>
                    </tr>
                ))}
            </tbody>
        </table>
    );
}
