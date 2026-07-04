import { useState, useEffect } from 'react';
import api from '../../services/api';
import ConfirmDialog from '../ConfirmDialog';
import { useToast } from '../../contexts/ToastContext';
import Modal from '../Modal';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import { Label } from '@/components/ui/label';

const SSHKeysTab = () => {
    const [keys, setKeys] = useState([]);
    const [loading, setLoading] = useState(true);
    const [showAddModal, setShowAddModal] = useState(false);
    const [newKey, setNewKey] = useState('');
    const [actionLoading, setActionLoading] = useState(false);
    const [confirmDialog, setConfirmDialog] = useState(null);
    const toast = useToast();

    useEffect(() => {
        loadKeys();
    }, []);

    const loadKeys = async () => {
        setLoading(true);
        try {
            const data = await api.getSSHKeys();
            setKeys(data.keys || []);
        } catch (error) {
            console.error('Failed to load SSH keys:', error);
        } finally {
            setLoading(false);
        }
    };

    const handleAddKey = async () => {
        if (!newKey.trim()) return;
        setActionLoading(true);
        try {
            await api.addSSHKey(newKey);
            toast.success('SSH key added successfully');
            setShowAddModal(false);
            setNewKey('');
            await loadKeys();
        } catch (error) {
            toast.error(`Failed to add key: ${error.message}`);
        } finally {
            setActionLoading(false);
        }
    };

    const handleRemoveKey = async (keyId, comment) => {
        setConfirmDialog({
            title: 'Remove SSH Key',
            message: `Are you sure you want to remove the SSH key${comment ? ` "${comment}"` : ''}? This may lock you out if it's your only key.`,
            confirmText: 'Remove',
            variant: 'danger',
            onConfirm: async () => {
                try {
                    await api.removeSSHKey(keyId);
                    toast.success('SSH key removed');
                    await loadKeys();
                } catch (error) {
                    toast.error(`Failed to remove key: ${error.message}`);
                }
                setConfirmDialog(null);
            },
            onCancel: () => setConfirmDialog(null)
        });
    };

    return (
        <div className="ssh-keys-tab">
            <div className="card sec-flush">
                <div className="card-header">
                    <h3>SSH Authorized Keys {!loading && keys.length > 0 && <span className="sec-count">· {keys.length}</span>}</h3>
                    <div className="card-actions">
                        <Button variant="default" size="sm" onClick={() => setShowAddModal(true)}>
                            Add Key
                        </Button>
                        <Button variant="outline" size="sm" onClick={loadKeys}>
                            Refresh
                        </Button>
                    </div>
                </div>
                {loading ? (
                    <div className="card-body">
                        <div className="loading-sm">Loading...</div>
                    </div>
                ) : keys.length === 0 ? (
                    <div className="card-body">
                        <div className="empty-state-sm">
                            <p>No SSH keys configured for root user.</p>
                            <Button variant="default" onClick={() => setShowAddModal(true)}>
                                Add SSH Key
                            </Button>
                        </div>
                    </div>
                ) : (
                    <table className="sk-dtable">
                        <thead>
                            <tr>
                                <th>Type</th>
                                <th>Fingerprint</th>
                                <th>Comment</th>
                                <th>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {keys.map((key) => (
                                <tr key={key.id}>
                                    <td><span className="sk-tag">{key.type}</span></td>
                                    <td className="sk-cell-mono sec-fp">{key.fingerprint}</td>
                                    <td>{key.comment || <span className="sec-dash">—</span>}</td>
                                    <td>
                                        <Button variant="destructive" size="sm" onClick={() => handleRemoveKey(key.id, key.comment)}>
                                            Remove
                                        </Button>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                )}
            </div>

            <Modal open={showAddModal} onClose={() => setShowAddModal(false)} title="Add SSH Public Key" size="lg">
                <div className="form-group">
                    <Label>Public Key</Label>
                    <Textarea
                        value={newKey}
                        onChange={(e) => setNewKey(e.target.value)}
                        placeholder="ssh-rsa AAAA... user@host or ssh-ed25519 AAAA... user@host"
                        rows={4}
                    />
                    <p className="help-text">Paste your SSH public key (typically from ~/.ssh/id_rsa.pub or ~/.ssh/id_ed25519.pub)</p>
                </div>
                <div className="modal-footer">
                    <Button variant="outline" onClick={() => setShowAddModal(false)}>Cancel</Button>
                    <Button variant="default" onClick={handleAddKey} disabled={actionLoading || !newKey.trim()}>
                        {actionLoading ? 'Adding...' : 'Add Key'}
                    </Button>
                </div>
            </Modal>

            {confirmDialog && (
                <ConfirmDialog
                    title={confirmDialog.title}
                    message={confirmDialog.message}
                    confirmText={confirmDialog.confirmText}
                    variant={confirmDialog.variant}
                    onConfirm={confirmDialog.onConfirm}
                    onCancel={confirmDialog.onCancel}
                />
            )}
        </div>
    );
};

export default SSHKeysTab;
