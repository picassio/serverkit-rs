import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import { Zap, Clock, Webhook, Activity, Play } from 'lucide-react';

const TriggerNode = ({ data, selected }) => {
    const {
        triggerType = 'manual',
        label = 'Trigger',
        isActive = false,
        lastStatus = null,
        triggerConfig = {}
    } = data;

    const getIcon = () => {
        switch (triggerType) {
            case 'manual': return <Play size={16} />;
            case 'cron': return <Clock size={16} />;
            case 'webhook': return <Webhook size={16} />;
            case 'event': return <Zap size={16} />;
            default: return <Activity size={16} />;
        }
    };

    const statusClass = !isActive ? 'inactive'
        : lastStatus === 'success' ? 'success'
        : lastStatus === 'failed' ? 'error'
        : 'active';

    const getSublabel = () => {
        const typeName = triggerType.charAt(0).toUpperCase() + triggerType.slice(1);
        const status = isActive ? 'Active' : 'Disabled';

        if (triggerType === 'cron' && triggerConfig.cron) {
            return `${triggerConfig.cron} \u2022 ${status}`;
        }
        if (triggerType === 'webhook' && triggerConfig.webhook_id) {
            return `...${triggerConfig.webhook_id.slice(-8)} \u2022 ${status}`;
        }
        if (triggerType === 'event' && triggerConfig.eventType) {
            const eventLabels = {
                health_check_failed: 'Health Fail',
                high_cpu: 'High CPU',
                high_memory: 'High Memory',
                git_push: 'Git Push',
                app_stopped: 'App Stopped'
            };
            return `${eventLabels[triggerConfig.eventType] || triggerConfig.eventType} \u2022 ${status}`;
        }

        return `${typeName} \u2022 ${status}`;
    };

    return (
        <div className={`workflow-node node-trigger node-status-${statusClass} ${selected ? 'node-selected' : ''}`}>
            <div className={`node-icon node-icon-${triggerType}`}>
                {getIcon()}
            </div>
            <div className="node-content">
                <div className="node-label">{label}</div>
                <div className="node-sublabel">{getSublabel()}</div>
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

export default memo(TriggerNode);
