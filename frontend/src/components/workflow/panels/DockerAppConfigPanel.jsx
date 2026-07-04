import { useState } from 'react';
import { Server, Globe, Link, ExternalLink, Play, Square, RotateCw } from 'lucide-react';
import ConfigPanel from '../ConfigPanel';
import api from '../../../services/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectTrigger, SelectContent, SelectItem, SelectValue } from '@/components/ui/select';

const DockerAppConfigPanel = ({ node, onChange, onClose }) => {
    const data = node?.data || {};
    const isReal = data.isReal || data.appId;

    const [isActioning, setIsActioning] = useState(false);
    const [actionMessage, setActionMessage] = useState(null);
    const [ports, setPorts] = useState(data.ports || []);

    const handleChange = (field, value) => {
        onChange({ ...data, [field]: value });
    };

    // Actions for real apps
    const handleAction = async (action) => {
        if (!data.appId) return;

        setIsActioning(true);
        setActionMessage(null);

        try {
            switch (action) {
                case 'start':
                    await api.startApp(data.appId);
                    handleChange('status', 'running');
                    break;
                case 'stop':
                    await api.stopApp(data.appId);
                    handleChange('status', 'stopped');
                    break;
                case 'restart':
                    await api.restartApp(data.appId);
                    handleChange('status', 'running');
                    break;
                default:
                    return;
            }
            setActionMessage(`${action} successful`);
        } catch (error) {
            setActionMessage(`Failed to ${action}`);
        } finally {
            setIsActioning(false);
            setTimeout(() => setActionMessage(null), 3000);
        }
    };

    const openPrivateUrl = () => {
        if (data.privateUrl) {
            window.open(data.privateUrl, '_blank');
        }
    };

    const openAppDetails = () => {
        if (data.appId) {
            window.location.href = `/applications/${data.appId}`;
        }
    };

    const handleAddPort = () => {
        const newPorts = [...ports, ''];
        setPorts(newPorts);
        handleChange('ports', newPorts);
    };

    const handlePortChange = (index, value) => {
        const newPorts = [...ports];
        newPorts[index] = value;
        setPorts(newPorts);
        handleChange('ports', newPorts);
    };

    const handleRemovePort = (index) => {
        const newPorts = ports.filter((_, i) => i !== index);
        setPorts(newPorts);
        handleChange('ports', newPorts);
    };

    // Real app panel - shows info and actions
    if (isReal) {
        return (
            <ConfigPanel
                isOpen={!!node}
                title="Application"
                icon={Server}
                headerColor="#2496ed"
                onClose={onClose}
            >
                <div className="form-group">
                    <Label>Name</Label>
                    <div className="form-value">{data.name || 'Unknown'}</div>
                </div>

                <div className="form-group">
                    <Label>Status</Label>
                    <div className={`form-value status-badge status-${data.status}`}>
                        {data.status || 'unknown'}
                    </div>
                </div>

                {data.template && (
                    <div className="form-group">
                        <Label>Template</Label>
                        <div className="form-value">{data.template}</div>
                    </div>
                )}

                {data.port && (
                    <div className="form-group">
                        <Label>Port</Label>
                        <div className="form-value">{data.port}</div>
                    </div>
                )}

                {data.privateUrl && (
                    <div className="form-group">
                        <Label>Private URL</Label>
                        <div className="form-value form-value-link" onClick={openPrivateUrl}>
                            <Link size={14} />
                            {data.privateUrl}
                            <ExternalLink size={12} />
                        </div>
                    </div>
                )}

                {data.domains && data.domains.length > 0 && (
                    <div className="form-group">
                        <Label>Connected Domains</Label>
                        <div className="form-domains">
                            {data.domains.map((domain, idx) => (
                                <div key={idx} className="form-domain-item">
                                    <Globe size={14} />
                                    <span>{domain.name || domain}</span>
                                    {domain.ssl_enabled && (
                                        <span className="ssl-badge">SSL</span>
                                    )}
                                </div>
                            ))}
                        </div>
                    </div>
                )}

                <div className="form-group">
                    <Label>Actions</Label>
                    <div className="action-buttons">
                        <Button
                            variant="default"
                            size="sm"
                            onClick={() => handleAction('start')}
                            disabled={isActioning || data.status === 'running'}
                        >
                            <Play size={14} />
                            Start
                        </Button>
                        <Button
                            variant="outline"
                            size="sm"
                            onClick={() => handleAction('stop')}
                            disabled={isActioning || data.status === 'stopped'}
                        >
                            <Square size={14} />
                            Stop
                        </Button>
                        <Button
                            variant="secondary"
                            size="sm"
                            onClick={() => handleAction('restart')}
                            disabled={isActioning}
                        >
                            <RotateCw size={14} />
                            Restart
                        </Button>
                    </div>
                    {actionMessage && (
                        <div className="action-message">{actionMessage}</div>
                    )}
                </div>

                <div className="form-group">
                    <Button
                        variant="default"
                        className="btn-block"
                        onClick={openAppDetails}
                    >
                        <ExternalLink size={14} />
                        Open App Details
                    </Button>
                </div>
            </ConfigPanel>
        );
    }

    // Legacy panel for non-real apps (manual creation)
    return (
        <ConfigPanel
            isOpen={!!node}
            title="Docker App"
            icon={Server}
            headerColor="#2496ed"
            onClose={onClose}
        >
            <div className="form-group">
                <Label>Name</Label>
                <Input
                    type="text"
                    value={data.name || ''}
                    onChange={(e) => handleChange('name', e.target.value)}
                    placeholder="my-app"
                />
            </div>

            <div className="form-group">
                <Label>Image</Label>
                <Input
                    type="text"
                    value={data.image || ''}
                    onChange={(e) => handleChange('image', e.target.value)}
                    placeholder="nginx:latest"
                />
                <span className="form-hint">Docker image name and tag</span>
            </div>

            <div className="form-group">
                <Label>Status</Label>
                <Select
                    value={data.status || 'stopped'}
                    onValueChange={(value) => handleChange('status', value)}
                >
                    <SelectTrigger>
                        <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                        <SelectItem value="stopped">Stopped</SelectItem>
                        <SelectItem value="running">Running</SelectItem>
                        <SelectItem value="error">Error</SelectItem>
                    </SelectContent>
                </Select>
            </div>

            <div className="form-group">
                <Label>Port Mappings</Label>
                <div className="port-list">
                    {ports.map((port, index) => (
                        <div key={index} className="port-item">
                            <Input
                                type="text"
                                value={port}
                                onChange={(e) => handlePortChange(index, e.target.value)}
                                placeholder="8080:80"
                            />
                            <Button
                                type="button"
                                variant="ghost"
                                size="icon"
                                onClick={() => handleRemovePort(index)}
                            >
                                ×
                            </Button>
                        </div>
                    ))}
                    <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={handleAddPort}
                    >
                        + Add Port
                    </Button>
                </div>
                <span className="form-hint">Format: host_port:container_port</span>
            </div>

            <div className="form-group">
                <Label>Memory Limit</Label>
                <Input
                    type="text"
                    value={data.memory || ''}
                    onChange={(e) => handleChange('memory', e.target.value)}
                    placeholder="512MB"
                />
                <span className="form-hint">e.g., 256MB, 1GB</span>
            </div>
        </ConfigPanel>
    );
};

export default DockerAppConfigPanel;
