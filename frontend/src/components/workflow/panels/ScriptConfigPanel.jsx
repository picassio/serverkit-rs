import ConfigPanel from '../ConfigPanel';
import { Terminal, Code } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';

const ScriptConfigPanel = ({ node, onChange, onClose, onDelete }) => {
    const { data } = node;
    const {
        language = 'bash',
        label = 'Run Script',
        content = '',
        timeout = 300,
        retryCount = 0,
        retryDelay = 5
    } = data;

    return (
        <ConfigPanel
            title="Script"
            icon={<Terminal size={16} />}
            color="#646b7a"
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
                <Label>Language</Label>
                <div className="lang-toggle">
                    <button type="button"
                        className={`lang-btn ${language === 'bash' ? 'active' : ''}`}
                        onClick={() => onChange({ ...data, language: 'bash' })}
                    >
                        <Terminal size={14} />
                        Bash
                    </button>
                    <button type="button"
                        className={`lang-btn ${language === 'python' ? 'active' : ''}`}
                        onClick={() => onChange({ ...data, language: 'python' })}
                    >
                        <Code size={14} />
                        Python
                    </button>
                </div>
            </div>

            <div className="form-group">
                <Label>Script Content</Label>
                <Textarea
                    className="script-editor font-mono"
                    value={content}
                    onChange={(e) => onChange({ ...data, content: e.target.value })}
                    placeholder={language === 'bash'
                        ? "#!/bin/bash\necho 'Hello World'"
                        : "print('Hello World')"
                    }
                    rows={12}
                />
            </div>

            <div className="form-row form-row-3">
                <div className="form-group">
                    <Label>Timeout (s)</Label>
                    <Input
                        type="number"
                        min="1"
                        max="3600"
                        value={timeout}
                        onChange={(e) => onChange({ ...data, timeout: parseInt(e.target.value) || 300 })}
                    />
                </div>
                <div className="form-group">
                    <Label>Retries</Label>
                    <Input
                        type="number"
                        min="0"
                        max="5"
                        value={retryCount}
                        onChange={(e) => onChange({ ...data, retryCount: parseInt(e.target.value) || 0 })}
                    />
                </div>
                <div className="form-group">
                    <Label>Delay (s)</Label>
                    <Input
                        type="number"
                        min="1"
                        max="300"
                        value={retryDelay}
                        onChange={(e) => onChange({ ...data, retryDelay: parseInt(e.target.value) || 5 })}
                    />
                </div>
            </div>

            <div className="panel-info-box">
                <strong>Variables</strong><br />
                <code>{'${node_id.stdout}'}</code> — output from a previous node<br />
                <code>$WORKFLOW_ID</code>, <code>$EXECUTION_ID</code> — workflow metadata<br />
                <code>$NODE_ID_OUTPUT</code> — node output as env var
            </div>
        </ConfigPanel>
    );
};

export default ScriptConfigPanel;
