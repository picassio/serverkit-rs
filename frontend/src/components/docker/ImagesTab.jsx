import { useState, useEffect } from 'react';
import api from '../../services/api';
import { useToast } from '../../contexts/ToastContext';
import { useConfirm } from '../../hooks/useConfirm';
import EmptyState from '../EmptyState';
import Modal from '@/components/Modal';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Layers } from 'lucide-react';
import {
    useServer,
    normalizeListResponse,
} from './dockerHelpers';
import { IconAction, TrashIcon } from './dockerShared';

export const PullImageButton = () => {
    const [showModal, setShowModal] = useState(false);
    return (
        <>
            <Button onClick={() => setShowModal(true)}>
                <span>+</span> Pull Image
            </Button>
            {showModal && <PullImageModal onClose={() => setShowModal(false)} onPulled={() => window.location.reload()} />}
        </>
    );
};

// Images Tab
const ImagesTab = ({ onStatsChange }) => {
    const toast = useToast();
    const { serverId, isRemote } = useServer();
    const { confirm: confirmImage } = useConfirm();
    const [images, setImages] = useState([]);
    const [loading, setLoading] = useState(true);
    const [searchTerm, setSearchTerm] = useState('');

    useEffect(() => {
        loadImages();
    }, [serverId]);

    async function loadImages() {
        setLoading(true);
        try {
            let data;
            if (isRemote) {
                const result = await api.getRemoteImages(serverId);
                data = { images: normalizeListResponse(result, 'images') };
            } else {
                data = await api.getImages();
            }
            setImages(data.images || []);
        } catch (err) {
            console.error('Failed to load images:', err);
        } finally {
            setLoading(false);
        }
    }

    async function handleRemove(imageId) {
        const confirmed = await confirmImage({ title: 'Remove Image', message: 'Remove this image?' });
        if (!confirmed) return;

        try {
            if (isRemote) {
                await api.removeRemoteImage(serverId, imageId, true);
            } else {
                await api.removeImage(imageId, true);
            }
            toast.success('Image removed successfully');
            loadImages();
            onStatsChange?.();
        } catch (err) {
            console.error('Failed to remove image:', err);
            toast.error('Failed to remove image. It may be in use by a container.');
        }
    }

    const filteredImages = images.filter(img => {
        if (!searchTerm) return true;
        const search = searchTerm.toLowerCase();
        return img.repository?.toLowerCase().includes(search) ||
               img.tag?.toLowerCase().includes(search) ||
               img.id?.toLowerCase().includes(search);
    });

    if (loading) {
        return <div className="docker-loading">Loading images...</div>;
    }

    return (
        <div>
            <div className="docker-table-header">
                <div />
                <Input
                    type="text"
                    className="docker-search"
                    placeholder="Search images..."
                    value={searchTerm}
                    onChange={(e) => setSearchTerm(e.target.value)}
                />
            </div>

            {filteredImages.length === 0 ? (
                <EmptyState
                    icon={Layers}
                    title="No images"
                    description="Pull an image to see it here."
                />
            ) : (
                <table className="docker-table">
                    <thead>
                        <tr>
                            <th>Repository</th>
                            <th>Tag</th>
                            <th>Image ID</th>
                            <th>Size</th>
                            <th>Created</th>
                            <th className="text-right">Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {filteredImages.map(image => (
                            <tr key={image.id}>
                                <td>
                                    <span className="docker-container-name">{image.repository || '<none>'}</span>
                                </td>
                                <td>
                                    <span className="docker-image-tag">{image.tag || '<none>'}</span>
                                </td>
                                <td>
                                    <span className="docker-container-id">{image.id?.substring(0, 12)}</span>
                                </td>
                                <td>{image.size}</td>
                                <td>{image.created}</td>
                                <td className="docker-actions-cell">
                                    <IconAction title="Delete" onClick={() => handleRemove(image.id)} color="#EF4444">
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

const PullImageModal = ({ onClose, onPulled }) => {
    const { serverId, isRemote } = useServer();
    const [image, setImage] = useState('');
    const [tag, setTag] = useState('latest');
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');

    async function handleSubmit(e) {
        e.preventDefault();
        setError('');
        setLoading(true);

        try {
            if (isRemote) {
                const fullImage = tag ? `${image}:${tag}` : image;
                await api.pullRemoteImage(serverId, fullImage);
            } else {
                await api.pullImage(image, tag);
            }
            onPulled();
            onClose();
        } catch (err) {
            setError(err.message || 'Failed to pull image');
        } finally {
            setLoading(false);
        }
    }

    return (
        <Modal open onClose={onClose} title="Pull Image" size="md">
            {error && <div className="error-message">{error}</div>}

            <form onSubmit={handleSubmit}>
                <div className="form-group">
                    <label>Image Name *</label>
                    <Input
                        type="text"
                        value={image}
                        onChange={(e) => setImage(e.target.value)}
                        placeholder="nginx, mysql, redis"
                        required
                    />
                </div>

                <div className="form-group">
                    <label>Tag</label>
                    <Input
                        type="text"
                        value={tag}
                        onChange={(e) => setTag(e.target.value)}
                        placeholder="latest"
                    />
                </div>

                <div className="modal-actions">
                    <Button type="button" variant="outline" onClick={onClose}>
                        Cancel
                    </Button>
                    <Button type="submit" disabled={loading}>
                        {loading ? 'Pulling...' : 'Pull Image'}
                    </Button>
                </div>
            </form>
        </Modal>
    );
};

export default ImagesTab;
