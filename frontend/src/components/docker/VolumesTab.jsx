import { useState, useEffect } from 'react';
import api from '../../services/api';
import { useToast } from '../../contexts/ToastContext';
import { useConfirm } from '../../hooks/useConfirm';
import EmptyState from '../EmptyState';
import Modal from '@/components/Modal';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { HardDrive } from 'lucide-react';
import {
    useServer,
    normalizeListResponse,
} from './dockerHelpers';
import { IconAction, TrashIcon } from './dockerShared';

export const CreateVolumeButton = () => {
    const [showModal, setShowModal] = useState(false);
    const { isRemote } = useServer();
    return (
        <>
            <Button
                onClick={() => setShowModal(true)}
                disabled={isRemote}
                title={isRemote ? 'Creating volumes is only available on the local Docker target right now' : 'Create volume'}
            >
                <span>+</span> Create Volume
            </Button>
            {showModal && <CreateVolumeModal onClose={() => setShowModal(false)} onCreated={() => window.location.reload()} />}
        </>
    );
};

// Volumes Tab
const VolumesTab = ({ onStatsChange }) => {
    const toast = useToast();
    const { serverId, isRemote } = useServer();
    const { confirm: confirmVolume } = useConfirm();
    const [volumes, setVolumes] = useState([]);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        loadVolumes();
    }, [serverId]);

    async function loadVolumes() {
        setLoading(true);
        try {
            let data;
            if (isRemote) {
                const result = await api.getRemoteVolumes(serverId);
                data = { volumes: normalizeListResponse(result, 'volumes') };
            } else {
                data = await api.getVolumes();
            }
            setVolumes(data.volumes || []);
        } catch (err) {
            console.error('Failed to load volumes:', err);
        } finally {
            setLoading(false);
        }
    }

    async function handleRemove(volumeName) {
        const confirmed = await confirmVolume({ title: 'Remove Volume', message: 'Remove this volume? All data will be lost.' });
        if (!confirmed) return;

        try {
            if (isRemote) {
                await api.removeRemoteVolume(serverId, volumeName, true);
            } else {
                await api.removeVolume(volumeName, true);
            }
            toast.success('Volume removed successfully');
            loadVolumes();
            onStatsChange?.();
        } catch (err) {
            console.error('Failed to remove volume:', err);
            toast.error('Failed to remove volume. It may be in use.');
        }
    }

    if (loading) {
        return <div className="docker-loading">Loading volumes...</div>;
    }

    return (
        <div>
            {volumes.length === 0 ? (
                <EmptyState
                    icon={HardDrive}
                    title="No volumes"
                    description="Create a volume for persistent data storage."
                />
            ) : (
                <table className="docker-table">
                    <thead>
                        <tr>
                            <th>Name</th>
                            <th>Driver</th>
                            <th>Mountpoint</th>
                            <th className="text-right">Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {volumes.map(volume => (
                            <tr key={volume.name}>
                                <td>
                                    <span className="docker-container-name">{volume.name}</span>
                                </td>
                                <td>{volume.driver}</td>
                                <td>
                                    <span className="docker-container-id truncate inline-block" style={{ maxWidth: '300px' }}>
                                        {volume.mountpoint || '-'}
                                    </span>
                                </td>
                                <td className="docker-actions-cell">
                                    <IconAction title="Delete" onClick={() => handleRemove(volume.name)} color="#EF4444">
                                        <TrashIcon />
                                    </IconAction>
                                </td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            )}
        </div>
    );
};

const CreateVolumeModal = ({ onClose, onCreated }) => {
    const [name, setName] = useState('');
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');

    async function handleSubmit(e) {
        e.preventDefault();
        setError('');
        setLoading(true);

        try {
            await api.createVolume(name);
            onCreated();
            onClose();
        } catch (err) {
            setError(err.message || 'Failed to create volume');
        } finally {
            setLoading(false);
        }
    }

    return (
        <Modal open onClose={onClose} title="Create Volume" size="md">
            {error && <div className="error-message">{error}</div>}

            <form onSubmit={handleSubmit}>
                <div className="form-group">
                    <label>Volume Name *</label>
                    <Input
                        type="text"
                        value={name}
                        onChange={(e) => setName(e.target.value)}
                        placeholder="my-volume"
                        required
                    />
                </div>

                <div className="modal-actions">
                    <Button type="button" variant="outline" onClick={onClose}>
                        Cancel
                    </Button>
                    <Button type="submit" disabled={loading}>
                        {loading ? 'Creating...' : 'Create Volume'}
                    </Button>
                </div>
            </form>
        </Modal>
    );
};

export default VolumesTab;
