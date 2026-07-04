import { useState, useEffect, useCallback } from 'react';
import { useTopbarActions } from '@/hooks/useTopbarActions';
import api from '../services/api';
import { useToast } from '../contexts/ToastContext';
import { useAuth } from '../contexts/AuthContext';
import PageLoader from '../components/PageLoader';
import ConfirmDialog from '../components/ConfirmDialog';
import EmptyState from '../components/EmptyState';
import Modal from '@/components/Modal';
import { LayoutTemplate } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Badge } from '@/components/ui/badge';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';

const ServerTemplates = () => {
    const toast = useToast();
    const { user } = useAuth();
    const [templates, setTemplates] = useState([]);
    const [library, setLibrary] = useState({});
    const [compliance, setCompliance] = useState(null);
    const [loading, setLoading] = useState(true);
    const [showCreateModal, setShowCreateModal] = useState(false);
    const [showAssignModal, setShowAssignModal] = useState(false);
    const [selectedTemplate, setSelectedTemplate] = useState(null);
    const [selectedDetail, setSelectedDetail] = useState(null);
    const [deleteConfirm, setDeleteConfirm] = useState(null);
    const [servers, setServers] = useState([]);

    const [form, setForm] = useState({
        name: '', description: '', category: 'general',
        packages: '', services: [], firewall_rules: [],
        auto_remediate: false, remediation_approval_required: true
    });

    const loadData = useCallback(async () => {
        try {
            const [tData, lData, cData] = await Promise.all([
                api.getServerTemplates(),
                api.getServerTemplateLibrary(),
                api.getTemplateCompliance(),
            ]);
            setTemplates(tData.templates || []);
            setLibrary(lData.templates || {});
            setCompliance(cData);
        } catch (err) {
            toast.error('Failed to load templates');
        } finally {
            setLoading(false);
        }
    }, [toast]);

    useEffect(() => {
        loadData();
        api.getServers().then(d => setServers(d.servers || [])).catch(() => {});
    }, [loadData]);

    const handleCreate = async () => {
        try {
            const data = {
                ...form,
                packages: form.packages.split('\n').map(p => p.trim()).filter(Boolean),
            };
            await api.createServerTemplate(data);
            toast.success('Template created');
            setShowCreateModal(false);
            loadData();
        } catch (err) {
            toast.error(err.message);
        }
    };

    const handleCreateFromLibrary = async (key) => {
        try {
            await api.createServerTemplateFromLibrary(key);
            toast.success('Template created from library');
            loadData();
        } catch (err) {
            toast.error(err.message);
        }
    };

    const handleDelete = async (id) => {
        try {
            await api.deleteServerTemplate(id);
            toast.success('Template deleted');
            setDeleteConfirm(null);
            loadData();
        } catch (err) {
            toast.error(err.message);
        }
    };

    const handleAssign = async (templateId, serverId) => {
        try {
            await api.assignServerTemplate(templateId, serverId);
            toast.success('Template assigned');
            setShowAssignModal(false);
            loadData();
        } catch (err) {
            toast.error(err.message);
        }
    };

    const handleCheckDrift = async (assignmentId) => {
        try {
            await api.checkTemplateDrift(assignmentId);
            toast.success('Drift check initiated');
            loadData();
        } catch (err) {
            toast.error(err.message);
        }
    };

    const handleRemediate = async (assignmentId) => {
        try {
            await api.remediateTemplateDrift(assignmentId);
            toast.success('Remediation initiated');
            loadData();
        } catch (err) {
            toast.error(err.message);
        }
    };

    const categoryLabels = {
        general: 'General', web: 'Web Server', database: 'Database', mail: 'Mail Server', custom: 'Custom'
    };

    // Publish the admin "Create Template" action to the shared tab-group top bar.
    useTopbarActions(() =>
        user?.is_admin ? (
            <Button size="sm" onClick={() => setShowCreateModal(true)}>Create Template</Button>
        ) : null,
        [user?.is_admin]
    );

    if (loading) return <PageLoader />;

    return (
        <div className="sk-tabgroup__inner server-templates-page">
            {compliance && (
                <div className="compliance-bar">
                    <div className="compliance-bar__stats">
                        <div className="stat-item stat-item--success">
                            <span className="stat-item__value">{compliance.compliant}</span>
                            <span className="stat-item__label">Compliant</span>
                        </div>
                        <div className="stat-item stat-item--danger">
                            <span className="stat-item__value">{compliance.drifted}</span>
                            <span className="stat-item__label">Drifted</span>
                        </div>
                        <div className="stat-item stat-item--muted">
                            <span className="stat-item__value">{compliance.unknown}</span>
                            <span className="stat-item__label">Unknown</span>
                        </div>
                    </div>
                    <div className="compliance-bar__progress">
                        <div className="progress-fill" style={{ width: `${compliance.compliance_pct}%` }} />
                    </div>
                </div>
            )}

            <Tabs defaultValue="templates">
                <TabsList>
                    <TabsTrigger value="templates">Templates</TabsTrigger>
                    <TabsTrigger value="library">Library</TabsTrigger>
                </TabsList>

                <TabsContent value="templates">
                    <div className="templates-grid">
                        {templates.map(tmpl => (
                            <div key={tmpl.id} className="template-card card" onClick={() => setSelectedDetail(selectedDetail?.id === tmpl.id ? null : tmpl)}>
                                <div className="template-card__header">
                                    <h3>{tmpl.name}</h3>
                                    <Badge variant="outline">{categoryLabels[tmpl.category] || tmpl.category}</Badge>
                                </div>
                                {tmpl.description && <p className="template-card__desc">{tmpl.description}</p>}
                                <div className="template-card__meta">
                                    <span>v{tmpl.version}</span>
                                    <span>{tmpl.assignment_count} server{tmpl.assignment_count !== 1 ? 's' : ''}</span>
                                    {tmpl.parent_name && <span>Inherits: {tmpl.parent_name}</span>}
                                </div>
                                <div className="template-card__spec">
                                    {tmpl.packages?.length > 0 && <span>{tmpl.packages.length} packages</span>}
                                    {tmpl.services?.length > 0 && <span>{tmpl.services.length} services</span>}
                                    {tmpl.firewall_rules?.length > 0 && <span>{tmpl.firewall_rules.length} firewall rules</span>}
                                </div>
                                <div className="template-card__actions" onClick={e => e.stopPropagation()}>
                                    <Button size="sm" onClick={() => { setSelectedTemplate(tmpl); setShowAssignModal(true); }}>
                                        Assign
                                    </Button>
                                    {user?.is_admin && (
                                        <Button size="sm" variant="destructive" onClick={() => setDeleteConfirm(tmpl)}>Delete</Button>
                                    )}
                                </div>
                            </div>
                        ))}
                        {templates.length === 0 && (
                            <EmptyState
                                icon={LayoutTemplate}
                                title="No templates yet"
                                description="Create one or use a library template to get started."
                            />
                        )}
                    </div>
                </TabsContent>

                <TabsContent value="library">
                    <div className="templates-grid">
                        {Object.entries(library).map(([key, tmpl]) => (
                            <div key={key} className="template-card card">
                                <div className="template-card__header">
                                    <h3>{tmpl.name}</h3>
                                    <Badge variant="outline">{categoryLabels[tmpl.category] || tmpl.category}</Badge>
                                </div>
                                <p className="template-card__desc">{tmpl.description}</p>
                                <div className="template-card__spec">
                                    {tmpl.packages?.length > 0 && <span>{tmpl.packages.length} packages</span>}
                                    {tmpl.services?.length > 0 && <span>{tmpl.services.length} services</span>}
                                    {tmpl.firewall_rules?.length > 0 && <span>{tmpl.firewall_rules.length} firewall rules</span>}
                                </div>
                                <div className="template-card__actions">
                                    <Button size="sm" onClick={() => handleCreateFromLibrary(key)}>
                                        Use Template
                                    </Button>
                                </div>
                            </div>
                        ))}
                    </div>
                </TabsContent>
            </Tabs>

            {/* Create Modal */}
            <Modal
                open={showCreateModal}
                onClose={() => setShowCreateModal(false)}
                title="Create Template"
                footer={(
                    <>
                        <Button variant="outline" onClick={() => setShowCreateModal(false)}>Cancel</Button>
                        <Button onClick={handleCreate} disabled={!form.name}>Create</Button>
                    </>
                )}
            >
                <div className="form-group">
                    <label>Name</label>
                    <Input value={form.name} onChange={e => setForm({...form, name: e.target.value})} />
                </div>
                <div className="form-group">
                    <label>Description</label>
                    <Textarea value={form.description} onChange={e => setForm({...form, description: e.target.value})} rows={2} />
                </div>
                <div className="form-group">
                    <label>Category</label>
                    <select className="form-select" value={form.category} onChange={e => setForm({...form, category: e.target.value})}>
                        {Object.entries(categoryLabels).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
                    </select>
                </div>
                <div className="form-group">
                    <label>Packages (one per line)</label>
                    <Textarea className="form-input--mono" value={form.packages} onChange={e => setForm({...form, packages: e.target.value})} rows={4} placeholder={"nginx\nphp-fpm\ncertbot"} />
                </div>
                <div className="form-group">
                    <label className="checkbox-label">
                        <input type="checkbox" checked={form.auto_remediate} onChange={e => setForm({...form, auto_remediate: e.target.checked})} />
                        Auto-remediate drift
                    </label>
                </div>
            </Modal>

            {/* Assign Modal */}
            <Modal
                open={Boolean(showAssignModal && selectedTemplate)}
                onClose={() => setShowAssignModal(false)}
                title={selectedTemplate ? `Assign ${selectedTemplate.name}` : 'Assign template'}
            >
                {selectedTemplate && (
                    <>
                        <p>Select a server to apply this template:</p>
                        <div className="server-select-list">
                            {servers.map(server => (
                                <div key={server.id} className="server-select-item" onClick={() => handleAssign(selectedTemplate.id, server.id)}>
                                    <span className={`status-dot status-dot--${server.status === 'online' ? 'success' : 'danger'}`} />
                                    <span>{server.name}</span>
                                </div>
                            ))}
                        </div>
                    </>
                )}
            </Modal>

            {deleteConfirm && (
                <ConfirmDialog
                    title="Delete Template"
                    message={`Delete "${deleteConfirm.name}"? This cannot be undone.`}
                    onConfirm={() => handleDelete(deleteConfirm.id)}
                    onCancel={() => setDeleteConfirm(null)}
                    variant="danger"
                />
            )}
        </div>
    );
};

export default ServerTemplates;
