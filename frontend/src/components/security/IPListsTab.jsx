import { useState, useEffect } from 'react';
import api from '../../services/api';
import ConfirmDialog from '../ConfirmDialog';
import { useToast } from '../../contexts/ToastContext';
import Modal from '../Modal';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

const IPListsTab = () => {
    const [lists, setLists] = useState({ allowlist: [], blocklist: [] });
    const [loading, setLoading] = useState(true);
    const [showAddModal, setShowAddModal] = useState(null);
    const [newIP, setNewIP] = useState('');
    const [newComment, setNewComment] = useState('');
    const [actionLoading, setActionLoading] = useState(false);
    const [confirmDialog, setConfirmDialog] = useState(null);
    const toast = useToast();

    useEffect(() => {
        loadLists();
    }, []);

    const loadLists = async () => {
        setLoading(true);
        try {
            const data = await api.getIPLists();
            setLists({
                allowlist: data.allowlist || [],
                blocklist: data.blocklist || []
            });
        } catch (error) {
            console.error('Failed to load IP lists:', error);
        } finally {
            setLoading(false);
        }
    };

    const handleAdd = async () => {
        if (!newIP.trim()) return;
        setActionLoading(true);
        try {
            await api.addToIPList(newIP, showAddModal, newComment);
            toast.success(`IP added to ${showAddModal}`);
            setShowAddModal(null);
            setNewIP('');
            setNewComment('');
            await loadLists();
        } catch (error) {
            toast.error(`Failed to add IP: ${error.message}`);
        } finally {
            setActionLoading(false);
        }
    };

    const handleRemove = async (ip, listType) => {
        setConfirmDialog({
            title: `Remove from ${listType}`,
            message: `Are you sure you want to remove ${ip} from the ${listType}?`,
            confirmText: 'Remove',
            variant: 'warning',
            onConfirm: async () => {
                try {
                    await api.removeFromIPList(ip, listType);
                    toast.success(`IP removed from ${listType}`);
                    await loadLists();
                } catch (error) {
                    toast.error(`Failed to remove IP: ${error.message}`);
                }
                setConfirmDialog(null);
            },
            onCancel: () => setConfirmDialog(null)
        });
    };

    const renderList = (title, listType, items, tone) => (
        <div className="card sec-flush">
            <div className="card-header">
                <h3 className={`sec-listtitle sec-listtitle--${tone}`}>
                    {title} <span className="sec-count">· {items.length}</span>
                </h3>
                <Button variant="default" size="sm" onClick={() => setShowAddModal(listType)}>
                    Add IP
                </Button>
            </div>
            {items.length === 0 ? (
                <div className="card-body">
                    <p className="text-muted">No IPs in {listType}.</p>
                </div>
            ) : (
                <table className="sk-dtable">
                    <thead>
                        <tr>
                            <th>IP / CIDR</th>
                            <th>Comment</th>
                            <th>Added</th>
                            <th></th>
                        </tr>
                    </thead>
                    <tbody>
                        {items.map((item, index) => (
                            <tr key={index}>
                                <td className={`sk-cell-mono sec-ip--${tone}`}>{item.ip}</td>
                                <td>{item.comment || <span className="sec-dash">—</span>}</td>
                                <td className="sk-cell-mono sec-faint">{new Date(item.added_at).toLocaleDateString()}</td>
                                <td className="sec-rowend">
                                    <Button variant="destructive" size="sm" onClick={() => handleRemove(item.ip, listType)}>
                                        Remove
                                    </Button>
                                </td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            )}
        </div>
    );

    if (loading) {
        return <div className="loading-sm">Loading IP lists...</div>;
    }

    return (
        <div className="ip-lists-tab">
            <div className="ip-lists-grid">
                {renderList('Allowlist', 'allowlist', lists.allowlist, 'green')}
                {renderList('Blocklist', 'blocklist', lists.blocklist, 'red')}
            </div>

            <Modal open={!!showAddModal} onClose={() => setShowAddModal(null)} title={`Add to ${showAddModal || ''}`}>
                <div className="form-group">
                    <Label>IP Address or CIDR</Label>
                    <Input
                        type="text"
                        value={newIP}
                        onChange={(e) => setNewIP(e.target.value)}
                        placeholder="192.168.1.100 or 10.0.0.0/24"
                    />
                </div>
                <div className="form-group">
                    <Label>Comment (optional)</Label>
                    <Input
                        type="text"
                        value={newComment}
                        onChange={(e) => setNewComment(e.target.value)}
                        placeholder="Office IP, VPN, etc."
                    />
                </div>
                <div className="modal-footer">
                    <Button variant="outline" onClick={() => setShowAddModal(null)}>Cancel</Button>
                    <Button variant="default" onClick={handleAdd} disabled={actionLoading || !newIP.trim()}>
                        {actionLoading ? 'Adding...' : 'Add'}
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

export default IPListsTab;
