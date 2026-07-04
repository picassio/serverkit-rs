import ConfigPanel from '../ConfigPanel';
import { GitBranch } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

const LogicIfConfigPanel = ({ node, onChange, onClose, onDelete }) => {
    const { data } = node;
    const { condition = '', label = 'If/Else' } = data;

    return (
        <ConfigPanel
            title="Logic (If/Else)"
            icon={<GitBranch size={16} />}
            color="#f97316"
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
                <Label>Condition (Python expression)</Label>
                <Input
                    type="text"
                    className="font-mono"
                    value={condition}
                    onChange={(e) => onChange({ ...data, condition: e.target.value })}
                    placeholder="e.g. results['node_1']['returncode'] == 0"
                />
                <span className="form-hint">Must evaluate to truthy or falsy. Errors follow the false branch.</span>
            </div>

            <div className="panel-info-box">
                <strong>Available Variables</strong><br />
                <code>results</code> — dict of previous node outputs, keyed by node ID<br />
                <code>context</code> — trigger execution context<br /><br />
                <strong>Examples</strong><br />
                <code>{"results['abc123']['returncode'] == 0"}</code><br />
                <code>{"context.get('event_type') == 'high_cpu'"}</code>
            </div>

            <div className="branch-legend">
                <div className="branch-legend-item">
                    <span className="branch-dot branch-dot-true" />
                    <span>True branch — condition is truthy</span>
                </div>
                <div className="branch-legend-item">
                    <span className="branch-dot branch-dot-false" />
                    <span>False branch — condition is falsy or errors</span>
                </div>
            </div>
        </ConfigPanel>
    );
};

export default LogicIfConfigPanel;
