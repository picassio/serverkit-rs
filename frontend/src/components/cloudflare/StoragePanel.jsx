import { useState, useEffect, useCallback } from 'react';
import { HardDrive } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import EmptyState from '../EmptyState';
import { useToast } from '../../contexts/ToastContext';
import api from '../../services/api';

function StorageSection({
    title,
    items,
    error,
    placeholder,
    hint,
    renderItem,
    onCreate,
    onDelete,
    isAdmin,
    busy,
}) {
    const [value, setValue] = useState('');

    const writeDisabled = !isAdmin || busy;

    const handleCreate = async () => {
        const trimmed = value.trim();
        if (!trimmed) return;
        // Only clear the input when the create succeeds; a throw leaves the text intact.
        await onCreate(trimmed);
        setValue('');
    };

    return (
        <section className="cf-storage__section">
            <h3 className="cf-storage__heading">
                {title} ({error ? 0 : items.length})
            </h3>

            {hint && <p className="cf-storage__hint">{hint}</p>}

            {error ? (
                <p className="cf-storage__error">{error}</p>
            ) : (
                <>
                    <div className="cf-storage__create">
                        <Input
                            value={value}
                            placeholder={placeholder}
                            onChange={(e) => setValue(e.target.value)}
                            disabled={writeDisabled}
                        />
                        <Button
                            size="sm"
                            onClick={handleCreate}
                            disabled={writeDisabled || !value.trim()}
                        >
                            Add
                        </Button>
                    </div>

                    {items.length === 0 ? (
                        <p className="cf-storage__hint">{`No ${title.toLowerCase()} yet.`}</p>
                    ) : (
                        <ul className="cf-storage__list">
                            {items.map((item) => {
                                const { key, label } = renderItem(item);
                                return (
                                    <li className="cf-storage__item" key={key}>
                                        <code className="cf-storage__name">{label}</code>
                                        <Button
                                            variant="destructive"
                                            size="sm"
                                            onClick={() => onDelete(item)}
                                            disabled={writeDisabled}
                                        >
                                            Delete
                                        </Button>
                                    </li>
                                );
                            })}
                        </ul>
                    )}
                </>
            )}
        </section>
    );
}

export default function StoragePanel({ zoneId, isAdmin }) {
    const toast = useToast();

    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);
    const [data, setData] = useState(null);

    // Tracks any in-flight write so all write controls can disable
    const [working, setWorking] = useState(false);

    const loadData = useCallback(async () => {
        try {
            const res = await api.getCloudflareStorage(zoneId);
            setData(res);
            setError(null);
        } catch (err) {
            setError(err.message);
        }
    }, [zoneId]);

    useEffect(() => {
        let active = true;
        setLoading(true);
        (async () => {
            try {
                const res = await api.getCloudflareStorage(zoneId);
                if (!active) return;
                setData(res);
                setError(null);
            } catch (err) {
                if (active) setError(err.message);
            } finally {
                if (active) setLoading(false);
            }
        })();
        return () => {
            active = false;
        };
    }, [zoneId]);

    const handleCreateR2 = async (name) => {
        setWorking(true);
        try {
            await api.createCloudflareR2Bucket(zoneId, name);
            toast.success(`Created bucket "${name}"`);
            await loadData();
        } catch (err) {
            toast.error(err.message);
            throw err;
        } finally {
            setWorking(false);
        }
    };

    const handleDeleteR2 = async (bucket) => {
        setWorking(true);
        try {
            await api.deleteCloudflareR2Bucket(zoneId, bucket.name);
            toast.success('Deleted');
            await loadData();
        } catch (err) {
            toast.error(err.message);
        } finally {
            setWorking(false);
        }
    };

    const handleCreateKv = async (title) => {
        setWorking(true);
        try {
            await api.createCloudflareKvNamespace(zoneId, title);
            toast.success(`Created namespace "${title}"`);
            await loadData();
        } catch (err) {
            toast.error(err.message);
            throw err;
        } finally {
            setWorking(false);
        }
    };

    const handleDeleteKv = async (namespace) => {
        setWorking(true);
        try {
            await api.deleteCloudflareKvNamespace(zoneId, namespace.id);
            toast.success('Deleted');
            await loadData();
        } catch (err) {
            toast.error(err.message);
        } finally {
            setWorking(false);
        }
    };

    const handleCreateD1 = async (name) => {
        setWorking(true);
        try {
            await api.createCloudflareD1Database(zoneId, name);
            toast.success(`Created database "${name}"`);
            await loadData();
        } catch (err) {
            toast.error(err.message);
            throw err;
        } finally {
            setWorking(false);
        }
    };

    const handleDeleteD1 = async (database) => {
        setWorking(true);
        try {
            await api.deleteCloudflareD1Database(zoneId, database.uuid);
            toast.success('Deleted');
            await loadData();
        } catch (err) {
            toast.error(err.message);
        } finally {
            setWorking(false);
        }
    };

    if (loading) {
        return <div className="cf-storage__loading">Loading storage…</div>;
    }

    if (error) {
        return (
            <EmptyState
                icon={HardDrive}
                title="Storage unavailable"
                description={error}
            />
        );
    }

    const errors = data.errors || {};

    return (
        <div className="cf-storage">
            <StorageSection
                title="R2 buckets"
                items={data.r2 || []}
                error={errors.r2}
                placeholder="my-bucket"
                hint="R2 is S3-compatible, so a bucket here can back ServerKit backups later."
                renderItem={(b) => ({ key: b.name, label: b.name })}
                onCreate={handleCreateR2}
                onDelete={handleDeleteR2}
                isAdmin={isAdmin}
                busy={working}
            />

            <StorageSection
                title="KV namespaces"
                items={data.kv || []}
                error={errors.kv}
                placeholder="my-namespace"
                renderItem={(n) => ({ key: n.id, label: n.title })}
                onCreate={handleCreateKv}
                onDelete={handleDeleteKv}
                isAdmin={isAdmin}
                busy={working}
            />

            <StorageSection
                title="D1 databases"
                items={data.d1 || []}
                error={errors.d1}
                placeholder="my-database"
                renderItem={(d) => ({ key: d.uuid, label: d.name })}
                onCreate={handleCreateD1}
                onDelete={handleDeleteD1}
                isAdmin={isAdmin}
                busy={working}
            />
        </div>
    );
}
