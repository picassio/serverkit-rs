import { useState } from 'react';
import api from '../../services/api';
import { useToast } from '../../contexts/ToastContext';
import { useConfirm } from '../../hooks/useConfirm';
import { Button } from '@/components/ui/button';
import { useServer } from './dockerHelpers';

const PruneButton = ({ onPruned }) => {
    const toast = useToast();
    const { isRemote } = useServer();
    const [loading, setLoading] = useState(false);
    const { confirm } = useConfirm();

    async function handlePrune() {
        if (isRemote) {
            toast.error('Prune is only available on the local Docker target right now');
            return;
        }
        const confirmed = await confirm({ title: 'Docker Cleanup', message: 'Remove unused Docker resources? This will remove stopped containers, unused images, and unused networks.' });
        if (!confirmed) return;

        setLoading(true);
        try {
            await api.request('/docker/cleanup', { method: 'POST', body: {} });
            toast.success('Docker cleanup completed');
            onPruned?.();
        } catch {
            toast.error('Failed to cleanup Docker resources');
        } finally {
            setLoading(false);
        }
    }

    return (
        <>
            <Button
                variant="outline"
                size="sm"
                onClick={handlePrune}
                disabled={loading || isRemote}
                title={isRemote ? 'Prune is only available on the local Docker target right now' : 'Prune unused Docker resources'}
            >
                {loading ? 'Cleaning...' : 'Prune Unused'}
            </Button>
        </>
    );
};

export default PruneButton;
