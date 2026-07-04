import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import { Bell, MessageSquare, Mail, Slack } from 'lucide-react';

const NotificationNode = ({ data, selected }) => {
    const {
        channel = 'discord',
        label = 'Notify',
        message = ''
    } = data;

    const getIcon = () => {
        switch (channel) {
            case 'discord': return <MessageSquare size={16} />;
            case 'slack': return <Slack size={16} />;
            case 'email': return <Mail size={16} />;
            default: return <Bell size={16} />;
        }
    };

    return (
        <div className={`workflow-node node-notification ${selected ? 'node-selected' : ''}`}>
            <Handle
                type="target"
                position={Position.Top}
                id="input"
                className="handle-input"
            />

            <div className={`node-icon node-icon-${channel}`}>
                {getIcon()}
            </div>
            <div className="node-content">
                <div className="node-label">{label}</div>
                <div className="node-sublabel">
                    {channel.charAt(0).toUpperCase() + channel.slice(1)}
                </div>
            </div>

            <Handle
                type="source"
                position={Position.Bottom}
                id="output"
                className="handle-output"
            />
        </div>
    );
};

export default memo(NotificationNode);
