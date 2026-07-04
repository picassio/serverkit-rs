import { useState } from 'react';
import ConfigPanel from '../ConfigPanel';
import { Play, Clock, Webhook, Zap, Copy, Check } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Select, SelectTrigger, SelectContent, SelectItem, SelectValue } from '@/components/ui/select';

const TriggerConfigPanel = ({ node, onChange, onClose, onDelete }) => {
    const { data } = node;
    const { triggerType = 'manual', label = 'Trigger', isActive = true, triggerConfig = {} } = data;
    const [copied, setCopied] = useState(false);

    const handleTypeChange = (type) => {
        const typeLabels = {
            manual: 'Manual Trigger',
            cron: 'Scheduled Task',
            webhook: 'Webhook Trigger',
            event: 'Event Listener'
        };
        onChange({
            ...data,
            triggerType: type,
            label: typeLabels[type] || `${type} Trigger`
        });
    };

    const handleConfigChange = (key, value) => {
        onChange({
            ...data,
            triggerConfig: { ...triggerConfig, [key]: value }
        });
    };

    const toggleActive = () => {
        onChange({ ...data, isActive: !isActive });
    };

    const webhookUrl = triggerConfig.webhook_id
        ? `${window.location.origin}/api/v1/workflows/hooks/${triggerConfig.webhook_id}`
        : null;

    const copyWebhookUrl = () => {
        if (webhookUrl) {
            navigator.clipboard.writeText(webhookUrl);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        }
    };

    const triggerTypes = [
        { id: 'manual', icon: Play, label: 'Manual', desc: 'Run on demand via button or API' },
        { id: 'cron', icon: Clock, label: 'Schedule', desc: 'Run on a cron schedule' },
        { id: 'webhook', icon: Webhook, label: 'Webhook', desc: 'Triggered by HTTP POST' },
        { id: 'event', icon: Zap, label: 'Event', desc: 'React to system events' },
    ];

    return (
        <ConfigPanel
            title="Trigger"
            icon={<Play size={16} />}
            color="#49c7f0"
            onClose={onClose}
            footer={onDelete && (
                <Button variant="destructive" size="sm" className="btn-delete-node" onClick={onDelete}>
                    Remove Node
                </Button>
            )}
        >
            <div className="form-group">
                <Label>Label</Label>
                <Input
                    type="text"
                    value={label}
                    onChange={(e) => onChange({ ...data, label: e.target.value })}
                />
            </div>

            <div className="form-group">
                <Label>Trigger Type</Label>
                <div className="trigger-type-grid">
                    {triggerTypes.map(({ id, icon: Icon, label: typeLabel, desc }) => (
                        <button type="button"
                            key={id}
                            className={`trigger-type-btn ${triggerType === id ? 'active' : ''}`}
                            onClick={() => handleTypeChange(id)}
                        >
                            <Icon size={18} />
                            <span className="trigger-type-name">{typeLabel}</span>
                            <span className="trigger-type-desc">{desc}</span>
                        </button>
                    ))}
                </div>
            </div>

            <div className="form-group">
                <label className="toggle-label">
                    <span>Enabled</span>
                    <Switch checked={isActive} onCheckedChange={toggleActive} />
                </label>
            </div>

            {triggerType === 'cron' && (
                <div className="form-group">
                    <Label>Cron Expression</Label>
                    <Input
                        type="text"
                        className="font-mono"
                        value={triggerConfig.cron || '0 * * * *'}
                        onChange={(e) => handleConfigChange('cron', e.target.value)}
                        placeholder="e.g. 0 0 * * *"
                    />
                    <span className="form-hint">
                        Format: minute hour day month weekday. Examples: <code>*/5 * * * *</code> (every 5 min), <code>0 0 * * *</code> (daily midnight)
                    </span>
                </div>
            )}

            {triggerType === 'webhook' && (
                <div className="form-group">
                    <Label>Webhook URL</Label>
                    {webhookUrl ? (
                        <>
                            <div className="input-with-action">
                                <Input
                                    type="text"
                                    value={webhookUrl}
                                    readOnly
                                    className="input-readonly font-mono"
                                />
                                <Button variant="ghost" size="icon" className="input-action-btn" onClick={copyWebhookUrl} title="Copy URL">
                                    {copied ? <Check size={14} /> : <Copy size={14} />}
                                </Button>
                            </div>
                            <span className="form-hint">
                                Send a POST request to this URL to trigger the workflow. Request body is available as <code>context.body</code>.
                            </span>
                        </>
                    ) : (
                        <div className="panel-info-box panel-info-warning">
                            Save the workflow first to generate the webhook URL.
                        </div>
                    )}
                </div>
            )}

            {triggerType === 'event' && (
                <div className="form-group">
                    <Label>System Event</Label>
                    <Select
                        value={triggerConfig.eventType || 'health_check_failed'}
                        onValueChange={(value) => handleConfigChange('eventType', value)}
                    >
                        <SelectTrigger>
                            <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                            <SelectItem value="health_check_failed">Health Check Failed</SelectItem>
                            <SelectItem value="high_cpu">High CPU Usage (&gt;80%)</SelectItem>
                            <SelectItem value="high_memory">High Memory Usage (&gt;80%)</SelectItem>
                            <SelectItem value="git_push">Git Push Received</SelectItem>
                            <SelectItem value="app_stopped">Application Stopped</SelectItem>
                        </SelectContent>
                    </Select>
                    <span className="form-hint">
                        Event data is available as <code>context.event_data</code> in scripts and conditions.
                    </span>
                </div>
            )}

            {triggerType === 'manual' && (
                <div className="panel-info-box">
                    Click <strong>Execute</strong> in the toolbar or call the workflow API to run manually.
                </div>
            )}
        </ConfigPanel>
    );
};

export default TriggerConfigPanel;
