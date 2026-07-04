import ConfigPanel from '../ConfigPanel';
import { Bell, MessageSquare, Mail, Slack, Send, Globe } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';

const NotificationConfigPanel = ({ node, onChange, onClose, onDelete }) => {
    const { data } = node;
    const { channel = 'discord', label = 'Notify', message = '' } = data;

    const channels = [
        { id: 'discord', icon: MessageSquare, label: 'Discord' },
        { id: 'slack', icon: Slack, label: 'Slack' },
        { id: 'email', icon: Mail, label: 'Email' },
        { id: 'telegram', icon: Send, label: 'Telegram' },
        { id: 'webhook', icon: Globe, label: 'Webhook' },
        { id: 'system', icon: Bell, label: 'All Channels' },
    ];

    return (
        <ConfigPanel
            title="Notification"
            icon={<Bell size={16} />}
            color="#b07bf5"
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
                <Label>Channel</Label>
                <div className="channel-grid">
                    {channels.map(({ id, icon: Icon, label: chLabel }) => (
                        <button type="button"
                            key={id}
                            className={`channel-btn ${channel === id ? 'active' : ''}`}
                            onClick={() => onChange({ ...data, channel: id })}
                        >
                            <Icon size={14} />
                            <span>{chLabel}</span>
                        </button>
                    ))}
                </div>
            </div>

            <div className="form-group">
                <Label>Message Template</Label>
                <Textarea
                    value={message}
                    onChange={(e) => onChange({ ...data, message: e.target.value })}
                    placeholder={'Build finished for {{workflow_name}}\\nExit code: ${build.returncode}'}
                    rows={8}
                />
            </div>

            <div className="panel-info-box">
                <strong>Variables</strong><br />
                <code>{'{{workflow_name}}'}</code>, <code>{'{{execution_id}}'}</code>, <code>{'{{started_at}}'}</code><br />
                <code>{'${node_id.stdout}'}</code> — output from a node<br />
                <code>{'{{context.field}}'}</code> — trigger context value
            </div>

            <div className="panel-info-box panel-info-warning">
                Channels must be configured in <strong>Settings &rarr; Notifications</strong> before they can send messages.
            </div>
        </ConfigPanel>
    );
};

export default NotificationConfigPanel;
