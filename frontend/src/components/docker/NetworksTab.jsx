import { useState, useEffect } from 'react';
import api from '../../services/api';
import { useToast } from '../../contexts/ToastContext';
import { useConfirm } from '../../hooks/useConfirm';
import EmptyState from '../EmptyState';
import Modal from '@/components/Modal';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Network as NetworkIcon } from 'lucide-react';
import {
    useServer,
    normalizeListResponse,
} from './dockerHelpers';
import { IconAction, TrashIcon } from './dockerShared';

export const CreateNetworkButton = () => {
    const [showModal, setShowModal] = useState(false);
    const { isRemote } = useServer();
    return (
        <>
            <Button
                onClick={() => setShowModal(true)}
                disabled={isRemote}
                title={isRemote ? 'Creating networks is only available on the local Docker target right now' : 'Create network'}
            >
                <span>+</span> Create Network
            </Button>
            {showModal && <CreateNetworkModal onClose={() => setShowModal(false)} onCreated={() => window.location.reload()} />}
        </>
    );
};

// Networks Tab
const NetworksTab = ({ onStatsChange }) => {
    const toast = useToast();
    const { serverId, isRemote } = useServer();
    const { confirm: confirmNetwork } = useConfirm();
    const [networks, setNetworks] = useState([]);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        loadNetworks();
    }, [serverId]);

    async function loadNetworks() {
        setLoading(true);
        try {
            let data;
            if (isRemote) {
                const result = await api.getRemoteNetworks(serverId);
                data = { networks: normalizeListResponse(result, 'networks') };
            } else {
                data = await api.getNetworks();
            }
            setNetworks(data.networks || []);
        } catch (err) {
            console.error('Failed to load networks:', err);
        } finally {
            setLoading(false);
        }
    }

    async function handleRemove(networkId) {
        const confirmed = await confirmNetwork({ title: 'Remove Network', message: 'Remove this network?' });
        if (!confirmed) return;

        try {
            if (isRemote) {
                await api.removeRemoteNetwork(serverId, networkId);
            } else {
                await api.removeNetwork(networkId);
            }
            toast.success('Network removed successfully');
            loadNetworks();
            onStatsChange?.();
        } catch (err) {
            console.error('Failed to remove network:', err);
            toast.error('Failed to remove network. It may be in use.');
        }
    }

    const systemNetworks = ['bridge', 'host', 'none'];

    if (loading) {
        return <div className="docker-loading">Loading networks...</div>;
    }

    return (
        <div>
            {networks.length === 0 ? (
                <EmptyState
                    icon={NetworkIcon}
                    title="No networks"
                    description="Create a network to connect containers."
                />
            ) : (
                <table className="docker-table">
                    <thead>
                        <tr>
                            <th>Name</th>
                            <th>Network ID</th>
                            <th>Driver</th>
                            <th>Scope</th>
                            <th className="text-right">Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {networks.map(network => (
                            <tr key={network.id}>
                                <td>
                                    <span className="docker-container-name">{network.name}</span>
                                </td>
                                <td>
                                    <span className="docker-container-id">{network.id?.substring(0, 12)}</span>
                                </td>
                                <td>{network.driver}</td>
                                <td>{network.scope}</td>
                                <td className="docker-actions-cell">
                                    {!systemNetworks.includes(network.name) && (
                                        <IconAction title="Delete" onClick={() => handleRemove(network.id)} color="#EF4444">
                                            <TrashIcon />
                                        </IconAction>
                                    )}
                                </td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            )}
        </div>
    );
};

const CreateNetworkModal = ({ onClose, onCreated }) => {
    const [name, setName] = useState('');
    const [driver, setDriver] = useState('bridge');
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');

    async function handleSubmit(e) {
        e.preventDefault();
        setError('');
        setLoading(true);

        try {
            await api.createNetwork(name, driver);
            onCreated();
            onClose();
        } catch (err) {
            setError(err.message || 'Failed to create network');
        } finally {
            setLoading(false);
        }
    }

    return (
        <Modal open onClose={onClose} title="Create Network" size="md">
            {error && <div className="error-message">{error}</div>}

            <form onSubmit={handleSubmit}>
                <div className="form-group">
                    <label>Network Name *</label>
                    <Input
                        type="text"
                        value={name}
                        onChange={(e) => setName(e.target.value)}
                        placeholder="my-network"
                        required
                    />
                </div>

                <div className="form-group">
                    <label>Driver</label>
                    <select value={driver} onChange={(e) => setDriver(e.target.value)}>
                        <option value="bridge">bridge</option>
                        <option value="overlay">overlay</option>
                        <option value="macvlan">macvlan</option>
                    </select>
                </div>

                <div className="modal-actions">
                    <Button type="button" variant="outline" onClick={onClose}>
                        Cancel
                    </Button>
                    <Button type="submit" disabled={loading}>
                        {loading ? 'Creating...' : 'Create Network'}
                    </Button>
                </div>
            </form>
        </Modal>
    );
};

export default NetworksTab;
