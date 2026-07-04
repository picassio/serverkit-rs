import { useState, useEffect, useCallback } from 'react';
import api from '../../services/api';
import { useToast } from '../../contexts/ToastContext';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Checkbox } from '@/components/ui/checkbox';

const RESOURCE_TYPES = ['application', 'database', 'service', 'wordpress', 'server'];

/**
 * Manage shared variable groups for a given scope: create groups, add/remove
 * variables (secrets masked), and attach/detach a group to resources.
 *
 * Props:
 *   scopeType   'workspace' | 'project' | 'environment'  (default 'workspace')
 *   scopeId     the scope identifier (string)            (default 'default')
 */
const SharedVariableGroups = ({ scopeType = 'workspace', scopeId = 'default' }) => {
    const toast = useToast();
    const [groups, setGroups] = useState([]);
    const [loading, setLoading] = useState(true);
    const [selectedId, setSelectedId] = useState(null);
    const [detail, setDetail] = useState(null);

    // create-group form
    const [newName, setNewName] = useState('');
    const [newDescription, setNewDescription] = useState('');

    // add-variable form
    const [varKey, setVarKey] = useState('');
    const [varValue, setVarValue] = useState('');
    const [varSecret, setVarSecret] = useState(false);
    const [varTarget, setVarTarget] = useState('');

    // attach form
    const [attachType, setAttachType] = useState(RESOURCE_TYPES[0]);
    const [attachId, setAttachId] = useState('');

    const loadGroups = useCallback(async () => {
        try {
            setLoading(true);
            const data = await api.listVariableGroups(scopeType, scopeId);
            setGroups(data.groups || []);
        } catch {
            toast.error('Failed to load variable groups');
        } finally {
            setLoading(false);
        }
    }, [scopeType, scopeId, toast]);

    const loadDetail = useCallback(async (groupId) => {
        if (!groupId) { setDetail(null); return; }
        try {
            const data = await api.getVariableGroup(groupId);
            setDetail(data);
        } catch {
            toast.error('Failed to load group');
        }
    }, [toast]);

    useEffect(() => { loadGroups(); }, [loadGroups]);
    useEffect(() => { loadDetail(selectedId); }, [selectedId, loadDetail]);

    async function handleCreateGroup(e) {
        e.preventDefault();
        if (!newName.trim()) return;
        try {
            const group = await api.createVariableGroup({
                scopeType, scopeId, name: newName.trim(),
                description: newDescription.trim() || null,
            });
            toast.success('Group created');
            setNewName('');
            setNewDescription('');
            await loadGroups();
            setSelectedId(group.id);
        } catch (err) {
            toast.error(err.message || 'Failed to create group');
        }
    }

    async function handleDeleteGroup(groupId) {
        if (!confirm('Delete this variable group? Attachments will be removed.')) return;
        try {
            await api.deleteVariableGroup(groupId);
            toast.success('Group deleted');
            if (selectedId === groupId) setSelectedId(null);
            loadGroups();
        } catch (err) {
            toast.error(err.message || 'Failed to delete group');
        }
    }

    async function handleAddVariable(e) {
        e.preventDefault();
        if (!varKey.trim() || !selectedId) return;
        try {
            await api.addGroupVariable(selectedId, {
                key: varKey.trim(), value: varValue, isSecret: varSecret,
                targetService: varTarget.trim() || null,
            });
            setVarKey('');
            setVarValue('');
            setVarSecret(false);
            setVarTarget('');
            loadDetail(selectedId);
            loadGroups();
        } catch (err) {
            toast.error(err.message || 'Failed to add variable');
        }
    }

    async function handleDeleteVariable(variableId) {
        try {
            await api.deleteGroupVariable(selectedId, variableId);
            loadDetail(selectedId);
            loadGroups();
        } catch (err) {
            toast.error(err.message || 'Failed to delete variable');
        }
    }

    async function handleAttach(e) {
        e.preventDefault();
        if (!attachId.trim() || !selectedId) return;
        try {
            await api.attachVariableGroup(selectedId, attachType, attachId.trim());
            toast.success('Group attached');
            setAttachId('');
            loadDetail(selectedId);
            loadGroups();
        } catch (err) {
            toast.error(err.message || 'Failed to attach group');
        }
    }

    async function handleDetach(att) {
        try {
            await api.detachVariableGroup(selectedId, att.resource_type, att.resource_id);
            loadDetail(selectedId);
            loadGroups();
        } catch (err) {
            toast.error(err.message || 'Failed to detach');
        }
    }

    return (
        <div className="shared-groups">
            <div className="shared-groups__layout">
                {/* Group list + create */}
                <aside className="shared-groups__sidebar">
                    <form className="shared-groups__create" onSubmit={handleCreateGroup}>
                        <Input
                            type="text"
                            value={newName}
                            onChange={(e) => setNewName(e.target.value)}
                            placeholder="New group name"
                        />
                        <Input
                            type="text"
                            value={newDescription}
                            onChange={(e) => setNewDescription(e.target.value)}
                            placeholder="Description (optional)"
                        />
                        <Button type="submit" size="sm" disabled={!newName.trim()}>
                            Create group
                        </Button>
                    </form>

                    <ul className="shared-groups__list">
                        {loading ? (
                            <li className="shared-groups__hint">Loading…</li>
                        ) : groups.length === 0 ? (
                            <li className="shared-groups__hint">No groups in this scope yet</li>
                        ) : (
                            groups.map((g) => (
                                <li
                                    key={g.id}
                                    className={`shared-groups__item ${selectedId === g.id ? 'is-active' : ''}`}
                                    onClick={() => setSelectedId(g.id)}
                                >
                                    <div className="shared-groups__item-name">{g.name}</div>
                                    <div className="shared-groups__item-meta">
                                        {g.variable_count} vars · {g.attachment_count} attached
                                    </div>
                                </li>
                            ))
                        )}
                    </ul>
                </aside>

                {/* Detail */}
                <section className="shared-groups__detail">
                    {!detail ? (
                        <div className="shared-groups__empty">
                            Select a group to manage its variables and attachments.
                        </div>
                    ) : (
                        <>
                            <div className="shared-groups__detail-header">
                                <div>
                                    <h3>{detail.name}</h3>
                                    {detail.description && (
                                        <p className="shared-groups__desc">{detail.description}</p>
                                    )}
                                </div>
                                <Button
                                    variant="destructive"
                                    size="sm"
                                    onClick={() => handleDeleteGroup(detail.id)}
                                >
                                    Delete group
                                </Button>
                            </div>

                            {/* Variables */}
                            <h4 className="shared-groups__subhead">Variables</h4>
                            {(detail.variables || []).length === 0 ? (
                                <p className="shared-groups__hint">No variables yet.</p>
                            ) : (
                                <table className="shared-vars-table">
                                    <thead>
                                        <tr>
                                            <th>Key</th>
                                            <th>Value</th>
                                            <th>Target service</th>
                                            <th></th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {detail.variables.map((v) => (
                                            <tr key={v.id} className={v.is_secret ? 'is-secret' : ''}>
                                                <td className="shared-vars-table__key">{v.key}</td>
                                                <td className="shared-vars-table__value">{v.value}</td>
                                                <td className="shared-vars-table__target">
                                                    {v.target_service ? (
                                                        <span
                                                            className="env-target-chip"
                                                            title={`Applies only to the "${v.target_service}" service`}
                                                        >
                                                            &rarr; {v.target_service}
                                                        </span>
                                                    ) : (
                                                        <span className="shared-vars-table__target-all">all services</span>
                                                    )}
                                                </td>
                                                <td>
                                                    <button
                                                        type="button"
                                                        className="shared-tag__remove"
                                                        onClick={() => handleDeleteVariable(v.id)}
                                                        title="Delete variable"
                                                        aria-label={`Delete variable ${v.key}`}
                                                    >
                                                        &times;
                                                    </button>
                                                </td>
                                            </tr>
                                        ))}
                                    </tbody>
                                </table>
                            )}

                            <form className="shared-groups__add-var" onSubmit={handleAddVariable}>
                                <Input
                                    type="text"
                                    value={varKey}
                                    onChange={(e) => setVarKey(e.target.value)}
                                    placeholder="KEY"
                                    className="shared-groups__var-key"
                                />
                                <Input
                                    type={varSecret ? 'password' : 'text'}
                                    value={varValue}
                                    onChange={(e) => setVarValue(e.target.value)}
                                    placeholder="value"
                                    className="shared-groups__var-value"
                                />
                                <Input
                                    type="text"
                                    value={varTarget}
                                    onChange={(e) => setVarTarget(e.target.value)}
                                    placeholder="all services"
                                    className="shared-groups__var-target"
                                    title="Target compose service (leave blank to apply to all services)"
                                />
                                <label className="shared-groups__secret">
                                    <Checkbox checked={varSecret} onCheckedChange={setVarSecret} />
                                    <span>Secret</span>
                                </label>
                                <Button type="submit" size="sm" disabled={!varKey.trim()}>
                                    Add
                                </Button>
                            </form>

                            {/* Attachments */}
                            <h4 className="shared-groups__subhead">Attached resources</h4>
                            {(detail.attachments || []).length === 0 ? (
                                <p className="shared-groups__hint">Not attached to any resource yet.</p>
                            ) : (
                                <ul className="shared-groups__attachments">
                                    {detail.attachments.map((a) => (
                                        <li key={a.id} className="shared-groups__attachment">
                                            <span className="shared-groups__attachment-label">
                                                {a.resource_type}:{a.resource_id}
                                            </span>
                                            <button
                                                type="button"
                                                className="shared-tag__remove"
                                                onClick={() => handleDetach(a)}
                                                title="Detach"
                                                aria-label="Detach group"
                                            >
                                                &times;
                                            </button>
                                        </li>
                                    ))}
                                </ul>
                            )}

                            <form className="shared-groups__attach" onSubmit={handleAttach}>
                                <select
                                    value={attachType}
                                    onChange={(e) => setAttachType(e.target.value)}
                                    className="shared-groups__attach-type"
                                >
                                    {RESOURCE_TYPES.map((t) => (
                                        <option key={t} value={t}>{t}</option>
                                    ))}
                                </select>
                                <Input
                                    type="text"
                                    value={attachId}
                                    onChange={(e) => setAttachId(e.target.value)}
                                    placeholder="resource id"
                                    className="shared-groups__attach-id"
                                />
                                <Button type="submit" size="sm" disabled={!attachId.trim()}>
                                    Attach
                                </Button>
                            </form>
                        </>
                    )}
                </section>
            </div>
        </div>
    );
};

export default SharedVariableGroups;
