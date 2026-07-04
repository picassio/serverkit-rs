import { Database } from 'lucide-react';
import ConfigPanel from '../ConfigPanel';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectTrigger, SelectContent, SelectItem, SelectValue } from '@/components/ui/select';

// Engine brand colors stay literal; fallback is the redesign's database amber.
const dbTypeConfig = {
    mysql: { color: '#00758f', defaultPort: 3306 },
    postgresql: { color: '#336791', defaultPort: 5432 },
    mongodb: { color: '#4db33d', defaultPort: 27017 },
    redis: { color: '#dc382d', defaultPort: 6379 }
};

const DatabaseConfigPanel = ({ node, onChange, onClose }) => {
    const data = node?.data || {};
    const dbType = data.type || 'mysql';
    const headerColor = dbTypeConfig[dbType]?.color || '#f5b945';

    const handleChange = (field, value) => {
        const updates = { ...data, [field]: value };

        // Auto-update port when type changes
        if (field === 'type' && dbTypeConfig[value]) {
            updates.port = dbTypeConfig[value].defaultPort;
        }

        onChange(updates);
    };

    return (
        <ConfigPanel
            isOpen={!!node}
            title="Database"
            icon={Database}
            headerColor={headerColor}
            onClose={onClose}
        >
            <div className="form-group">
                <Label>Name</Label>
                <Input
                    type="text"
                    value={data.name || ''}
                    onChange={(e) => handleChange('name', e.target.value)}
                    placeholder="my-database"
                />
            </div>

            <div className="form-group">
                <Label>Type</Label>
                <Select
                    value={data.type || 'mysql'}
                    onValueChange={(value) => handleChange('type', value)}
                >
                    <SelectTrigger>
                        <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                        <SelectItem value="mysql">MySQL</SelectItem>
                        <SelectItem value="postgresql">PostgreSQL</SelectItem>
                        <SelectItem value="mongodb">MongoDB</SelectItem>
                        <SelectItem value="redis">Redis</SelectItem>
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

            <div className="form-row">
                <div className="form-group">
                    <Label>Host</Label>
                    <Input
                        type="text"
                        value={data.host || ''}
                        onChange={(e) => handleChange('host', e.target.value)}
                        placeholder="localhost"
                    />
                </div>

                <div className="form-group">
                    <Label>Port</Label>
                    <Input
                        type="number"
                        value={data.port || dbTypeConfig[dbType]?.defaultPort || 3306}
                        onChange={(e) => handleChange('port', parseInt(e.target.value) || '')}
                    />
                </div>
            </div>

            {data.size && (
                <div className="form-group">
                    <Label>Size</Label>
                    <Input
                        type="text"
                        value={data.size}
                        disabled
                        className="input-readonly"
                    />
                    <span className="form-hint">Database size (read-only)</span>
                </div>
            )}
        </ConfigPanel>
    );
};

export default DatabaseConfigPanel;
