import { useState, useEffect } from 'react';
import api from '../../services/api';
import { Button } from '@/components/ui/button';
import { formatBytes } from '@/utils/formatBytes';

const QuarantineTab = () => {
    const [files, setFiles] = useState([]);
    const [loading, setLoading] = useState(true);
    const [message, setMessage] = useState(null);

    useEffect(() => {
        loadFiles();
    }, []);

    async function loadFiles() {
        try {
            const data = await api.getQuarantinedFiles();
            setFiles(data.files || []);
        } catch (err) {
            console.error('Failed to load quarantined files:', err);
        } finally {
            setLoading(false);
        }
    }

    async function handleDelete(filename) {
        if (!confirm(`Permanently delete ${filename}? This cannot be undone.`)) return;

        try {
            await api.deleteQuarantinedFile(filename);
            setMessage({ type: 'success', text: 'File deleted' });
            loadFiles();
        } catch (err) {
            setMessage({ type: 'error', text: err.message });
        }
    }

    return (
        <div className="quarantine-tab">
            {message && (
                <div className={`alert alert-${message.type === 'success' ? 'success' : 'danger'}`}>
                    {message.text}
                </div>
            )}

            <div className="card sec-flush">
                <div className="card-header">
                    <h3>Quarantined Files {!loading && files.length > 0 && <span className="sec-count">· {files.length}</span>}</h3>
                    <Button variant="outline" size="sm" onClick={loadFiles}>Refresh</Button>
                </div>
                {loading ? (
                    <div className="card-body">
                        <div className="loading-sm">Loading...</div>
                    </div>
                ) : files.length === 0 ? (
                    <div className="card-body">
                        <div className="empty-state">
                            <svg viewBox="0 0 24 24" width="48" height="48" stroke="currentColor" fill="none" strokeWidth="1">
                                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
                                <polyline points="9 12 12 15 16 10"/>
                            </svg>
                            <p>No files in quarantine</p>
                            <span className="text-muted">Infected files will appear here when detected</span>
                        </div>
                    </div>
                ) : (
                    <table className="sk-dtable">
                        <thead>
                            <tr>
                                <th>Filename</th>
                                <th>Size</th>
                                <th>Quarantined</th>
                                <th>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {files.map((file, index) => (
                                <tr key={index}>
                                    <td className="sk-cell-mono sec-path sec-path--red">{file.name}</td>
                                    <td className="sk-cell-mono">{formatBytes(file.size, { defaultValue: '0 B' })}</td>
                                    <td className="sk-cell-mono sec-faint">{new Date(file.quarantined_at).toLocaleString()}</td>
                                    <td>
                                        <Button
                                            variant="destructive"
                                            size="sm"
                                            onClick={() => handleDelete(file.name)}
                                        >
                                            Delete
                                        </Button>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                )}
            </div>
        </div>
    );
};

export default QuarantineTab;
