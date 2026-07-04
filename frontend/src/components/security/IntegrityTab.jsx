import { useState } from 'react';
import api from '../../services/api';
import { Button } from '@/components/ui/button';

const IntegrityTab = () => {
    const [checking, setChecking] = useState(false);
    const [initializing, setInitializing] = useState(false);
    const [results, setResults] = useState(null);
    const [message, setMessage] = useState(null);

    async function handleInitialize() {
        if (!confirm('This will create a new baseline for file integrity monitoring. Continue?')) return;

        setInitializing(true);
        setMessage(null);
        try {
            const result = await api.initializeIntegrityDatabase();
            setMessage({ type: 'success', text: result.message });
        } catch (err) {
            setMessage({ type: 'error', text: err.message });
        } finally {
            setInitializing(false);
        }
    }

    async function handleCheck() {
        setChecking(true);
        setMessage(null);
        try {
            const result = await api.checkFileIntegrity();
            setResults(result);
            if (result.total_changes === 0) {
                setMessage({ type: 'success', text: 'No changes detected' });
            }
        } catch (err) {
            setMessage({ type: 'error', text: err.message });
        } finally {
            setChecking(false);
        }
    }

    return (
        <div className="integrity-tab">
            {message && (
                <div className={`alert alert-${message.type === 'success' ? 'success' : 'danger'}`}>
                    {message.text}
                </div>
            )}

            <div className="card">
                <div className="card-header">
                    <h3>File Integrity Monitoring</h3>
                </div>
                <div className="card-body">
                    <p className="description">
                        File integrity monitoring tracks changes to critical system files. Initialize a baseline database,
                        then periodically check for unauthorized modifications.
                    </p>

                    <div className="integrity-actions">
                        <Button
                            variant="outline"
                            onClick={handleInitialize}
                            disabled={initializing}
                        >
                            {initializing ? 'Initializing...' : 'Initialize Baseline'}
                        </Button>
                        <Button
                            variant="default"
                            onClick={handleCheck}
                            disabled={checking}
                        >
                            {checking ? 'Checking...' : 'Check Integrity'}
                        </Button>
                    </div>
                </div>
            </div>

            {results && results.total_changes > 0 && (() => {
                const changes = results.changes || {};
                const NEW_LIMIT = 50;
                const newFiles = changes.new || [];
                const extraNew = Math.max(0, newFiles.length - NEW_LIMIT);
                // Flatten every change kind into dense-table rows (kind chip + mono path).
                const rows = [
                    ...(changes.modified || []).map((f) => ({ kind: 'modified', tone: 'amber', path: f.path })),
                    ...(changes.deleted || []).map((f) => ({ kind: 'deleted', tone: 'red', path: f })),
                    ...newFiles.slice(0, NEW_LIMIT).map((f) => ({ kind: 'new', tone: 'green', path: f })),
                    ...(changes.permission_changed || []).map((f) => ({
                        kind: 'permission',
                        tone: 'cyan',
                        path: `${f.path} (${f.old_mode} → ${f.new_mode})`,
                    })),
                ];

                return (
                    <div className="card sec-flush">
                        <div className="card-header">
                            <h3>Changes Detected</h3>
                            <span className="sec-state sec-state--amber">{results.total_changes} changes</span>
                        </div>
                        <table className="sk-dtable">
                            <thead>
                                <tr>
                                    <th>Change</th>
                                    <th>Path</th>
                                </tr>
                            </thead>
                            <tbody>
                                {rows.map((row, i) => (
                                    <tr key={i}>
                                        <td><span className={`sec-state sec-state--${row.tone}`}>{row.kind}</span></td>
                                        <td className="sk-cell-mono sec-path">{row.path}</td>
                                    </tr>
                                ))}
                                {extraNew > 0 && (
                                    <tr>
                                        <td><span className="sec-state sec-state--green">new</span></td>
                                        <td className="sk-cell-mono sec-faint">… and {extraNew} more added</td>
                                    </tr>
                                )}
                            </tbody>
                        </table>
                    </div>
                );
            })()}
        </div>
    );
};

export default IntegrityTab;
