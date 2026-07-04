import { useState, useEffect } from 'react';
import api from '../../services/api';
import { useToast } from '../../contexts/ToastContext';
import { useConfirm } from '../../hooks/useConfirm';
import EmptyState from '../EmptyState';
import Modal from '@/components/Modal';
import { Button } from '@/components/ui/button';
import { Package, Play, Square, RotateCw, FileText } from 'lucide-react';
import {
    useServer,
    unwrapRemoteData,
    normalizeListResponse,
} from './dockerHelpers';
import { IconAction, DownloadIcon } from './dockerShared';

// Compose Tab
const ComposeTab = ({ onStatsChange }) => {
    const toast = useToast();
    const { serverId, isRemote } = useServer();
    const { confirm: confirmCompose } = useConfirm();
    const [projects, setProjects] = useState([]);
    const [loading, setLoading] = useState(true);
    const [logsProject, setLogsProject] = useState(null);
    const [actionLoading, setActionLoading] = useState({});

    useEffect(() => {
        loadProjects();
    }, [serverId]);

    async function loadProjects() {
        setLoading(true);
        try {
            let data;
            if (isRemote) {
                data = await api.getRemoteComposeProjects(serverId);
            } else {
                data = await api.composeList();
            }
            setProjects(normalizeListResponse(data, 'projects'));
        } catch (err) {
            console.error('Failed to load compose projects:', err);
            setProjects([]);
        } finally {
            setLoading(false);
        }
    }

    async function handleAction(project, action) {
        const projectPath = project.ConfigFiles || project.config_files;
        if (!projectPath) {
            toast.error('Project path not found');
            return;
        }

        setActionLoading(prev => ({ ...prev, [project.Name || project.name]: true }));

        try {
            if (action === 'up') {
                if (isRemote) {
                    await api.remoteComposeUp(serverId, projectPath);
                } else {
                    await api.composeUp(projectPath, true, false);
                }
                toast.success('Project started');
            } else if (action === 'down') {
                const downConfirmed = await confirmCompose({ title: 'Stop Compose Project', message: 'Stop this compose project? Containers will be removed.' });
                if (!downConfirmed) {
                    setActionLoading(prev => ({ ...prev, [project.Name || project.name]: false }));
                    return;
                }
                if (isRemote) {
                    await api.remoteComposeDown(serverId, projectPath);
                } else {
                    await api.composeDown(projectPath, false, true);
                }
                toast.success('Project stopped');
            } else if (action === 'restart') {
                if (isRemote) {
                    await api.remoteComposeRestart(serverId, projectPath);
                } else {
                    await api.composeRestart(projectPath);
                }
                toast.success('Project restarted');
            } else if (action === 'pull') {
                if (isRemote) {
                    await api.remoteComposePull(serverId, projectPath);
                } else {
                    await api.composePull(projectPath);
                }
                toast.success('Images pulled');
            }
            loadProjects();
            onStatsChange?.();
        } catch (err) {
            console.error(`Failed to ${action} project:`, err);
            toast.error(err.message || `Failed to ${action} project`);
        } finally {
            setActionLoading(prev => ({ ...prev, [project.Name || project.name]: false }));
        }
    }

    function getProjectStatus(project) {
        const status = project.Status || project.status || '';
        if (status.includes('running')) return 'running';
        if (status.includes('exited') || status.includes('stopped')) return 'exited';
        return 'unknown';
    }

    function parseRunningCount(status) {
        // Parse status like "running(3)" or "exited(2), running(1)"
        const match = status.match(/running\((\d+)\)/);
        return match ? parseInt(match[1], 10) : 0;
    }

    if (loading) {
        return <div className="docker-loading">Loading compose projects...</div>;
    }

    return (
        <div>
            <div className="docker-table-header">
                <div className="docker-table-info">
                    {projects.length} project{projects.length !== 1 ? 's' : ''} found
                </div>
                <Button variant="outline" size="sm" onClick={loadProjects}>
                    Refresh
                </Button>
            </div>

            {projects.length === 0 ? (
                <EmptyState
                    icon={Package}
                    title="No Compose projects"
                    description="No Docker Compose projects are running on this server."
                    action={<code>docker compose up -d</code>}
                />
            ) : (
                <table className="docker-table">
                    <thead>
                        <tr>
                            <th>Project</th>
                            <th>Status</th>
                            <th>Config File</th>
                            <th className="text-right">Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {projects.map(project => {
                            const name = project.Name || project.name;
                            const status = project.Status || project.status || 'unknown';
                            const configFiles = project.ConfigFiles || project.config_files || '';
                            const isRunning = getProjectStatus(project) === 'running';
                            const runningCount = parseRunningCount(status);
                            const isLoading = actionLoading[name];

                            return (
                                <tr key={name}>
                                    <td>
                                        <span className="docker-container-name">{name}</span>
                                    </td>
                                    <td>
                                        <span className={`docker-status-pill ${isRunning ? 'running' : 'exited'}`}>
                                            <span className="docker-status-dot" />
                                            {isRunning ? `Running (${runningCount})` : 'Stopped'}
                                        </span>
                                        <div className="docker-status-detail">{status}</div>
                                    </td>
                                    <td>
                                        <span className="docker-container-id truncate inline-block" style={{ maxWidth: '300px' }}>
                                            {configFiles}
                                        </span>
                                    </td>
                                    <td className="docker-actions-cell">
                                        <IconAction
                                            title="Logs"
                                            onClick={() => setLogsProject(project)}
                                            disabled={isLoading}
                                        >
                                            <FileText size={14} />
                                        </IconAction>
                                        {isRunning ? (
                                            <>
                                                <IconAction
                                                    title="Restart"
                                                    onClick={() => handleAction(project, 'restart')}
                                                    disabled={isLoading}
                                                >
                                                    <RotateCw size={14} />
                                                </IconAction>
                                                <IconAction
                                                    title="Stop"
                                                    onClick={() => handleAction(project, 'down')}
                                                    disabled={isLoading}
                                                    color="#EF4444"
                                                >
                                                    <Square size={14} />
                                                </IconAction>
                                            </>
                                        ) : (
                                            <IconAction
                                                title="Start"
                                                onClick={() => handleAction(project, 'up')}
                                                disabled={isLoading}
                                                color="#10B981"
                                            >
                                                <Play size={14} />
                                            </IconAction>
                                        )}
                                        <IconAction
                                            title="Pull Images"
                                            onClick={() => handleAction(project, 'pull')}
                                            disabled={isLoading}
                                        >
                                            <DownloadIcon />
                                        </IconAction>
                                    </td>
                                </tr>
                            );
                        })}
                    </tbody>
                </table>
            )}

            {logsProject && (
                <ComposeLogsModal
                    project={logsProject}
                    onClose={() => setLogsProject(null)}
                />
            )}
        </div>
    );
};

// Compose Logs Modal
const ComposeLogsModal = ({ project, onClose }) => {
    const { serverId, isRemote } = useServer();
    const [logs, setLogs] = useState('');
    const [loading, setLoading] = useState(true);
    const [tail, setTail] = useState(200);
    const [selectedService, setSelectedService] = useState('');
    const [services, setServices] = useState([]);

    const projectName = project.Name || project.name;
    const projectPath = project.ConfigFiles || project.config_files || '';

    useEffect(() => {
        loadServices();
        loadLogs();
    }, [project, tail, selectedService]);

    async function loadServices() {
        try {
            let containers;
            if (isRemote) {
                containers = normalizeListResponse(
                    await api.getRemoteComposePs(serverId, projectPath),
                    'containers'
                );
            } else {
                const result = await api.composePs(projectPath);
                containers = result.containers || result || [];
            }

            // Extract unique service names
            const serviceNames = [...new Set(
                (Array.isArray(containers) ? containers : [])
                    .map(c => c.Service || c.service)
                    .filter(Boolean)
            )];
            setServices(serviceNames);
        } catch (err) {
            console.error('Failed to load services:', err);
        }
    }

    async function loadLogs() {
        setLoading(true);
        try {
            let data;
            if (isRemote) {
                data = unwrapRemoteData(await api.remoteComposeLogs(serverId, projectPath, selectedService || null, tail));
            } else {
                data = await api.composeLogs(projectPath, selectedService || null, tail);
            }
            setLogs(data.logs || 'No logs available');
        } catch (err) {
            setLogs('Failed to load logs: ' + (err.message || 'Unknown error'));
        } finally {
            setLoading(false);
        }
    }

    return (
        <Modal open onClose={onClose} title={`Logs: ${projectName}`} size="lg">
            <div className="modal-body">
                <div className="logs-controls flex flex-wrap items-center gap-2 mb-2">
                    <label>Service:</label>
                    <select
                        value={selectedService}
                        onChange={(e) => setSelectedService(e.target.value)}
                        className="py-2 px-2"
                    >
                        <option value="">All Services</option>
                        {services.map(service => (
                            <option key={service} value={service}>{service}</option>
                        ))}
                    </select>
                    <label>Lines:</label>
                    <select value={tail} onChange={(e) => setTail(Number(e.target.value))} className="py-2 px-2">
                        <option value={50}>50</option>
                        <option value={100}>100</option>
                        <option value={200}>200</option>
                        <option value={500}>500</option>
                        <option value={1000}>1000</option>
                    </select>
                </div>
                <pre className="log-viewer">{loading ? 'Loading...' : logs}</pre>
            </div>
            <div className="modal-actions">
                <Button variant="outline" onClick={loadLogs} disabled={loading}>
                    {loading ? 'Loading...' : 'Refresh'}
                </Button>
                <Button onClick={onClose}>Close</Button>
            </div>
        </Modal>
    );
};

export default ComposeTab;
