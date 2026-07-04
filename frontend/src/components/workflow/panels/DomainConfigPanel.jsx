import { Globe } from 'lucide-react';
import ConfigPanel from '../ConfigPanel';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectTrigger, SelectContent, SelectItem, SelectValue } from '@/components/ui/select';

const DomainConfigPanel = ({ node, onChange, onClose }) => {
    const data = node?.data || {};

    const handleChange = (field, value) => {
        const updates = { ...data, [field]: value };

        // Clear expiry date if SSL is not valid
        if (field === 'ssl' && value !== 'valid') {
            updates.sslExpiry = '';
        }

        onChange(updates);
    };

    return (
        <ConfigPanel
            isOpen={!!node}
            title="Domain"
            icon={Globe}
            headerColor="#3ddc97"
            onClose={onClose}
        >
            <div className="form-group">
                <Label>Domain Name</Label>
                <Input
                    type="text"
                    value={data.name || ''}
                    onChange={(e) => handleChange('name', e.target.value)}
                    placeholder="example.com"
                />
                <span className="form-hint">Enter domain without http:// or https://</span>
            </div>

            <div className="form-group">
                <Label>SSL Status</Label>
                <Select
                    value={data.ssl || 'none'}
                    onValueChange={(value) => handleChange('ssl', value)}
                >
                    <SelectTrigger>
                        <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                        <SelectItem value="none">No SSL</SelectItem>
                        <SelectItem value="valid">Valid Certificate</SelectItem>
                        <SelectItem value="expired">Expired Certificate</SelectItem>
                    </SelectContent>
                </Select>
            </div>

            {data.ssl === 'valid' && (
                <div className="form-group">
                    <Label>SSL Expiry Date</Label>
                    <Input
                        type="date"
                        value={data.sslExpiry || ''}
                        onChange={(e) => handleChange('sslExpiry', e.target.value)}
                    />
                    <span className="form-hint">Certificate expiration date</span>
                </div>
            )}

            <div className="form-group">
                <Label>DNS Status</Label>
                <Select
                    value={data.dnsStatus || 'pending'}
                    onValueChange={(value) => handleChange('dnsStatus', value)}
                >
                    <SelectTrigger>
                        <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                        <SelectItem value="pending">Pending Propagation</SelectItem>
                        <SelectItem value="propagated">Propagated</SelectItem>
                    </SelectContent>
                </Select>
                <span className="form-hint">DNS record propagation status</span>
            </div>
        </ConfigPanel>
    );
};

export default DomainConfigPanel;
