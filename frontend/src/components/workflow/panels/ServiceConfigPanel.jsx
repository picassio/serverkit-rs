import { Box } from 'lucide-react';
import ConfigPanel from '../ConfigPanel';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Select, SelectTrigger, SelectContent, SelectItem, SelectValue } from '@/components/ui/select';

// Service brand colors stay literal; queue/fallback use the redesign's
// violet/accent.
const serviceTypeConfig = {
    redis: { color: '#dc382d', defaultPort: 6379 },
    memcached: { color: '#00a65a', defaultPort: 11211 },
    rabbitmq: { color: '#ff6600', defaultPort: 5672 },
    queue: { color: '#b07bf5', defaultPort: 5555 }
};

const ServiceConfigPanel = ({ node, onChange, onClose }) => {
    const data = node?.data || {};
    const serviceType = data.serviceType || 'redis';
    const headerColor = serviceTypeConfig[serviceType]?.color || '#6d7cff';

    const handleChange = (field, value) => {
        const updates = { ...data, [field]: value };

        // Auto-update port when service type changes
        if (field === 'serviceType' && serviceTypeConfig[value]) {
            updates.port = serviceTypeConfig[value].defaultPort;
        }

        onChange(updates);
    };

    return (
        <ConfigPanel
            isOpen={!!node}
            title="Service"
            icon={Box}
            headerColor={headerColor}
            onClose={onClose}
        >
            <div className="form-group">
                <Label>Name</Label>
                <Input
                    type="text"
                    value={data.name || ''}
                    onChange={(e) => handleChange('name', e.target.value)}
                    placeholder="my-service"
                />
            </div>

            <div className="form-group">
                <Label>Service Type</Label>
                <Select
                    value={data.serviceType || 'redis'}
                    onValueChange={(value) => handleChange('serviceType', value)}
                >
                    <SelectTrigger>
                        <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                        <SelectItem value="redis">Redis</SelectItem>
                        <SelectItem value="memcached">Memcached</SelectItem>
                        <SelectItem value="rabbitmq">RabbitMQ</SelectItem>
                        <SelectItem value="queue">Queue</SelectItem>
                    </SelectContent>
                </Select>
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
                <Label>Port</Label>
                <Input
                    type="number"
                    value={data.port || serviceTypeConfig[serviceType]?.defaultPort || 6379}
                    onChange={(e) => handleChange('port', parseInt(e.target.value) || '')}
                />
            </div>

            <div className="form-group">
                <Label>Description</Label>
                <Textarea
                    value={data.description || ''}
                    onChange={(e) => handleChange('description', e.target.value)}
                    placeholder="Service description..."
                    rows={3}
                />
            </div>
        </ConfigPanel>
    );
};

export default ServiceConfigPanel;
