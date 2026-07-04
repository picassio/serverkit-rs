import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import { Terminal, Code, Cpu } from 'lucide-react';

const ScriptNode = ({ data, selected }) => {
    const {
        language = 'bash',
        label = 'Run Script',
        content = ''
    } = data;

    const getIcon = () => {
        switch (language) {
            case 'bash': return <Terminal size={16} />;
            case 'python': return <Code size={16} />;
            default: return <Cpu size={16} />;
        }
    };

    return (
        <div className={`workflow-node node-script ${selected ? 'node-selected' : ''}`}>
            <Handle
                type="target"
                position={Position.Top}
                id="input"
                className="handle-input"
            />

            <div className={`node-icon node-icon-${language}`}>
                {getIcon()}
            </div>
            <div className="node-content">
                <div className="node-label">{label}</div>
                <div className="node-sublabel">
                    {language.charAt(0).toUpperCase() + language.slice(1)}
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

export default memo(ScriptNode);
