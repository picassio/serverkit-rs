import { useState, useEffect } from 'react';
import api from '../../services/api';
import { useToast } from '../../contexts/ToastContext';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import EmptyState from '../EmptyState';

const GunicornTab = ({ appId }) => {
    const toast = useToast();
    const [config, setConfig] = useState('');
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);

    useEffect(() => {
        loadConfig();
    }, [appId]);

    async function loadConfig() {
        try {
            const data = await api.getGunicornConfig(appId);
            setConfig(data.content || '');
        } catch (err) {
            console.error('Failed to load config:', err);
        } finally {
            setLoading(false);
        }
    }

    async function handleSave() {
        setSaving(true);
        try {
            await api.updateGunicornConfig(appId, config);
            toast.success('Configuration saved. Restart the app to apply changes.');
        } catch (err) {
            toast.error('Failed to save configuration');
        } finally {
            setSaving(false);
        }
    }

    if (loading) {
        return <EmptyState loading title="Loading Gunicorn configuration..." />;
    }

    return (
        <div>
            <div className="section-header">
                <h3 className="svc-eyebrow">Gunicorn Configuration</h3>
                <Button onClick={handleSave} disabled={saving}>
                    {saving ? 'Saving...' : 'Save'}
                </Button>
            </div>
            <Textarea
                className="code-editor"
                value={config}
                onChange={(e) => setConfig(e.target.value)}
                spellCheck={false}
            />
        </div>
    );
};

export default GunicornTab;
