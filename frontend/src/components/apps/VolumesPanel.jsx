import { useState, useEffect, useCallback } from 'react';
import { HardDrive, Plus, Trash2, AlertTriangle } from 'lucide-react';
import api from '../../services/api';
import { useToast } from '../../contexts/ToastContext';
import { Pill } from '../ds';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Checkbox } from '@/components/ui/checkbox';

function formatBytes(bytes) {
    if (bytes == null) return '—';
    if (bytes === 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
    return `${(bytes / 1024 ** i).toFixed(i ? 1 : 0)} ${units[i]}`;
}

// A single volume row with an inline detach confirm (the "also delete data"
// checkbox is off by default and disabled while the app is running, so a detach
// can never nuke live data by accident).
function VolumeRow({ volume, appRunning, onDetach }) {
    const [confirming, setConfirming] = useState(false);
    const [wipe, setWipe] = useState(false);
    const [busy, setBusy] = useState(false);

    async function detach() {
        setBusy(true);
        try {
            await onDetach(volume, wipe);
        } finally {
            setBusy(false);
            setConfirming(false);
            setWipe(false);
        }
    }

    return (
        <div className="app-volumes__row">
            <div className="app-volumes__cell app-volumes__cell--name">
                <strong>{volume.name}</strong>
                <span className="app-volumes__mono">{volume.mount_path}</span>
            </div>
            <div className="app-volumes__cell">{formatBytes(volume.size_bytes)}</div>
            <div className="app-volumes__cell app-volumes__pills">
                {volume.read_only && <Pill kind="gray">read-only</Pill>}
                <Pill kind={volume.present === false ? 'red' : 'green'}>
                    {volume.present === false ? 'missing' : 'present'}
                </Pill>
            </div>
            <div className="app-volumes__cell app-volumes__actions">
                {confirming ? (
                    <div className="app-volumes__confirm">
                        <label className="app-volumes__wipe">
                            <Checkbox
                                checked={wipe}
                                onCheckedChange={(v) => setWipe(Boolean(v))}
                                disabled={appRunning}
                            />
                            <span>Also delete data{appRunning ? ' (stop the app first)' : ' — cannot be undone'}</span>
                        </label>
                        <Button type="button" size="sm" variant="ghost" disabled={busy} onClick={() => setConfirming(false)}>Cancel</Button>
                        <Button type="button" size="sm" variant={wipe ? 'destructive' : 'default'} disabled={busy} onClick={detach}>
                            {busy ? 'Detaching…' : (wipe ? 'Detach + wipe' : 'Detach')}
                        </Button>
                    </div>
                ) : (
                    <Button type="button" size="sm" variant="outline" onClick={() => setConfirming(true)}>
                        <Trash2 size={15} /> Detach
                    </Button>
                )}
            </div>
        </div>
    );
}

const VolumesPanel = ({ app, onChanged }) => {
    const toast = useToast();
    const [volumes, setVolumes] = useState([]);
    const [loading, setLoading] = useState(true);
    const [form, setForm] = useState({ name: '', mount_path: '', read_only: false });
    const [attaching, setAttaching] = useState(false);

    const appRunning = app.status === 'running';

    const load = useCallback(async () => {
        setLoading(true);
        try {
            const data = await api.getAppVolumes(app.id);
            setVolumes(data?.volumes || []);
        } catch (err) {
            toast.error(err.message || 'Failed to load volumes');
        } finally {
            setLoading(false);
        }
    }, [app.id, toast]);

    useEffect(() => { load(); }, [load]);

    async function attach(e) {
        e.preventDefault();
        setAttaching(true);
        try {
            await api.attachAppVolume(app.id, {
                name: form.name.trim(),
                mount_path: form.mount_path.trim(),
                read_only: form.read_only,
            });
            toast.success('Volume attached. It mounts on the next deploy.');
            setForm({ name: '', mount_path: '', read_only: false });
            await load();
            onChanged?.();
        } catch (err) {
            toast.error(err.message || 'Failed to attach volume');
        } finally {
            setAttaching(false);
        }
    }

    async function detach(volume, wipe) {
        try {
            await api.detachAppVolume(app.id, volume.id, { wipe });
            toast.success(wipe ? 'Volume detached and data wiped.' : 'Volume detached (data preserved).');
            await load();
            onChanged?.();
        } catch (err) {
            toast.error(err.message || 'Failed to detach volume');
        }
    }

    const canAttach = form.name.trim() && form.mount_path.trim().startsWith('/');

    return (
        <div className="app-volumes">
            <div className="app-panel">
                <div className="app-panel-header">
                    <HardDrive />
                    <span>Managed Volumes</span>
                </div>
                <div className="app-panel-body">
                    <p className="app-panel-hint">
                        First-class persistent storage that survives redeploys. Each volume is a
                        named Docker volume mounted into the container at the path you choose —
                        safer than a relative bind mount. Changes apply on the next deploy.
                    </p>

                    {loading ? (
                        <p className="app-panel-hint">Loading volumes…</p>
                    ) : volumes.length === 0 ? (
                        <p className="app-volumes__empty">No managed volumes yet. Attach one below.</p>
                    ) : (
                        <div className="app-volumes__table">
                            <div className="app-volumes__row app-volumes__row--head">
                                <div className="app-volumes__cell">Name &amp; mount path</div>
                                <div className="app-volumes__cell">Size</div>
                                <div className="app-volumes__cell">Status</div>
                                <div className="app-volumes__cell" />
                            </div>
                            {volumes.map((v) => (
                                <VolumeRow key={v.id} volume={v} appRunning={appRunning} onDetach={detach} />
                            ))}
                        </div>
                    )}

                    <form className="app-volumes__attach" onSubmit={attach}>
                        <div className="app-volumes__attach-fields">
                            <div className="container-ops__input">
                                <Label htmlFor={`vol-name-${app.id}`}>Name</Label>
                                <Input
                                    id={`vol-name-${app.id}`}
                                    value={form.name}
                                    onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
                                    placeholder="uploads"
                                />
                            </div>
                            <div className="container-ops__input">
                                <Label htmlFor={`vol-path-${app.id}`}>Container mount path</Label>
                                <Input
                                    id={`vol-path-${app.id}`}
                                    value={form.mount_path}
                                    onChange={(e) => setForm((f) => ({ ...f, mount_path: e.target.value }))}
                                    placeholder="/var/www/html/wp-content/uploads"
                                />
                            </div>
                            <label className="app-volumes__ro">
                                <Checkbox
                                    checked={form.read_only}
                                    onCheckedChange={(v) => setForm((f) => ({ ...f, read_only: Boolean(v) }))}
                                />
                                <span>Read-only</span>
                            </label>
                        </div>
                        <div className="app-detail-actions">
                            <Button type="submit" size="sm" disabled={attaching || !canAttach}>
                                <Plus size={15} /> {attaching ? 'Attaching…' : 'Attach volume'}
                            </Button>
                        </div>
                    </form>

                    <p className="app-volumes__note">
                        <AlertTriangle size={14} /> Detaching keeps the data by default. Wiping is
                        only allowed while the app is stopped.
                    </p>
                </div>
            </div>
        </div>
    );
};

export default VolumesPanel;
