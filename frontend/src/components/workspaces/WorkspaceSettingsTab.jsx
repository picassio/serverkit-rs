import { useState, useEffect } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { Settings, PanelLeft, Check, Archive, ArchiveRestore, Trash2 } from 'lucide-react';
import { useToast } from '../../contexts/ToastContext';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Checkbox } from '@/components/ui/checkbox';
import { SIDEBAR_ITEMS } from '../sidebarItems';
import api from '../../services/api';

const SETTINGS_GROUPS = [
    {
        label: 'General',
        items: [
            { id: 'general', label: 'General', icon: Settings },
        ],
    },
    {
        label: 'Permissions',
        items: [
            { id: 'navigation', label: 'Navigation Permissions', icon: PanelLeft },
        ],
    },
    {
        label: 'Management',
        items: [
            { id: 'management', label: 'Workspace actions', icon: Archive },
        ],
    },
];

const SETTINGS_ITEMS = SETTINGS_GROUPS.flatMap((g) => g.items);

const GeneralSection = ({ form, setForm }) => (
    <div className="ws-settings__section">
        <h3 className="ws-settings__section-title">General</h3>
        <div className="card settings-section">
            <div className="form-group">
                <label>Name</label>
                <Input
                    value={form.name}
                    onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
                    placeholder="My Team"
                />
            </div>
            <div className="form-group">
                <label>Description</label>
                <Textarea
                    value={form.description}
                    onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
                    rows={2}
                />
            </div>
            <div className="form-row">
                <div className="form-group">
                    <label>Max Servers (0 = unlimited)</label>
                    <Input
                        type="number"
                        value={form.max_servers}
                        onChange={(e) => setForm((f) => ({ ...f, max_servers: parseInt(e.target.value) || 0 }))}
                    />
                </div>
                <div className="form-group">
                    <label>Max Users (0 = unlimited)</label>
                    <Input
                        type="number"
                        value={form.max_users}
                        onChange={(e) => setForm((f) => ({ ...f, max_users: parseInt(e.target.value) || 0 }))}
                    />
                </div>
            </div>
            <div className="form-group">
                <label>Brand Color</label>
                <input
                    type="color"
                    className="workspace-color-input"
                    value={form.primary_color}
                    onChange={(e) => setForm((f) => ({ ...f, primary_color: e.target.value }))}
                    aria-label="Workspace brand color"
                />
                <span className="form-hint">Recolors the panel for anyone viewing this workspace.</span>
            </div>
        </div>
    </div>
);

const ManagementSection = ({ ws, isCurrent, onSetActive, onArchive, onRestore, onDeleteClick, user }) => (
    <div className="ws-settings__section">
        <h3 className="ws-settings__section-title">Workspace management</h3>
        <div className="card settings-section ws-settings__management">
            <p className="form-hint">These actions take effect immediately and cannot be undone from this screen.</p>
            <div className="ws-settings__actions-grid">
                {!isCurrent && ws.status === 'active' && (
                    <Button variant="outline" onClick={onSetActive}>
                        <Check size={15} /> Set active workspace
                    </Button>
                )}
                {ws.status === 'active'
                    ? (
                        <Button variant="outline" onClick={onArchive}>
                            <Archive size={15} /> Archive workspace
                        </Button>
                    ) : (
                        <Button variant="outline" onClick={onRestore}>
                            <ArchiveRestore size={15} /> Restore workspace
                        </Button>
                    )}
                {user?.is_admin && (
                    <Button variant="destructive" onClick={onDeleteClick}>
                        <Trash2 size={15} /> Delete workspace
                    </Button>
                )}
            </div>
        </div>
    </div>
);

const NavigationPermissionsSection = ({ form, setForm }) => {
    const roles = ['owner', 'admin', 'member', 'viewer'];
    return (
        <div className="ws-settings__section">
            <h3 className="ws-settings__section-title">Navigation Permissions</h3>
            <div className="card settings-section">
                <p className="form-hint">Limit which sidebar items each workspace role can see. Empty = no restrictions.</p>
                {roles.map((role) => (
                    <div key={role} className="ws-nav-role">
                        <div className="ws-nav-role__label">{role}</div>
                        <div className="ws-nav-role__items">
                            {SIDEBAR_ITEMS.filter((item) => !item.alwaysVisible).map((item) => {
                                const checked = (form.nav[role] || []).includes(item.id);
                                return (
                                    <label key={`${role}-${item.id}`} className="ws-nav-role__item">
                                        <Checkbox
                                            checked={checked}
                                            onCheckedChange={(val) => {
                                                const list = new Set(form.nav[role] || []);
                                                if (val) list.add(item.id);
                                                else list.delete(item.id);
                                                setForm((f) => ({ ...f, nav: { ...f.nav, [role]: Array.from(list) } }));
                                            }}
                                        />
                                        <span>{item.label}</span>
                                    </label>
                                );
                            })}
                        </div>
                    </div>
                ))}
            </div>
        </div>
    );
};

const WorkspaceSettingsTab = ({ wsId, ws, onUpdate, user, isCurrent, onSetActive, onArchive, onRestore, onDeleteClick }) => {
    const navigate = useNavigate();
    const toast = useToast();
    const { id, section: sectionParam } = useParams();
    const section = SETTINGS_ITEMS.some((s) => s.id === sectionParam) ? sectionParam : 'general';
    const setSection = (s) => navigate(`/workspaces/${id}/settings/${s}`, { replace: true });
    const active = SETTINGS_ITEMS.find((s) => s.id === section) || SETTINGS_ITEMS[0];

    useEffect(() => {
        if (!sectionParam || !SETTINGS_ITEMS.some((s) => s.id === sectionParam)) {
            navigate(`/workspaces/${id}/settings/general`, { replace: true });
        }
    }, [id, sectionParam, navigate]);

    const [form, setForm] = useState({
        name: ws.name,
        description: ws.description || '',
        max_servers: ws.max_servers || 0,
        max_users: ws.max_users || 0,
        primary_color: ws.primary_color || '#6d7cff',
        nav: ws.settings?.nav || { owner: [], admin: [], member: [], viewer: [] },
    });
    const [saving, setSaving] = useState(false);

    const handleSave = async () => {
        setSaving(true);
        try {
            const payload = {
                name: form.name,
                description: form.description,
                max_servers: form.max_servers,
                max_users: form.max_users,
                primary_color: form.primary_color,
                settings: { nav: form.nav },
            };
            await api.updateWorkspace(wsId, payload);
            toast.success('Workspace updated');
            onUpdate();
        } catch (err) {
            toast.error(err.message || 'Failed to update workspace');
        } finally {
            setSaving(false);
        }
    };

    return (
        <div className="ws-settings">
            <nav className="ws-settings__nav" aria-label="Workspace settings sections">
                {SETTINGS_GROUPS.map((g) => (
                    <div className="ws-settings__group" key={g.label}>
                        <div className="ws-settings__grouplabel">{g.label}</div>
                        {g.items.map((s) => (
                            <button
                                type="button"
                                key={s.id}
                                className={`ws-settings__navitem ${section === s.id ? 'is-active' : ''}`}
                                onClick={() => setSection(s.id)}
                            >
                                <s.icon size={15} />
                                {s.label}
                            </button>
                        ))}
                    </div>
                ))}
            </nav>
            <div className="ws-settings__content">
                {active.id === 'general' && <GeneralSection form={form} setForm={setForm} />}
                {active.id === 'navigation' && <NavigationPermissionsSection form={form} setForm={setForm} />}
                {active.id === 'management' && (
                    <ManagementSection
                        ws={ws}
                        isCurrent={isCurrent}
                        onSetActive={onSetActive}
                        onArchive={onArchive}
                        onRestore={onRestore}
                        onDeleteClick={onDeleteClick}
                        user={user}
                    />
                )}
                {active.id !== 'management' && (
                    <div className="ws-settings__actions">
                        <Button variant="outline" onClick={() => navigate(`/workspaces/${id}`)}>Cancel</Button>
                        <Button onClick={handleSave} disabled={saving || !form.name}>
                            {saving ? 'Saving…' : 'Save'}
                        </Button>
                    </div>
                )}
            </div>
        </div>
    );
};

export default WorkspaceSettingsTab;
