import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import { GitBranch } from 'lucide-react';

const LogicIfNode = ({ data, selected }) => {
    const {
        condition = '',
        label = 'If/Else'
    } = data;

    return (
        <div className={`workflow-node node-logic ${selected ? 'node-selected' : ''}`}>
            <Handle
                type="target"
                position={Position.Top}
                id="input"
                className="handle-input"
            />

            <div className="node-icon node-icon-logic">
                <GitBranch size={16} />
            </div>
            <div className="node-content">
                <div className="node-label">{label}</div>
                <div className="node-sublabel">
                    {condition || 'No condition'}
                </div>
            </div>

            <div className="node-outputs">
                <div className="node-output-branch">
                    <span className="node-branch-label node-branch-true">TRUE</span>
                    <Handle
                        type="source"
                        position={Position.Bottom}
                        id="true"
                        className="handle-output handle-true"
                        style={{ left: '25%' }}
                    />
                </div>
                <div className="node-output-branch">
                    <span className="node-branch-label node-branch-false">FALSE</span>
                    <Handle
                        type="source"
                        position={Position.Bottom}
                        id="false"
                        className="handle-output handle-false"
                        style={{ left: '75%' }}
                    />
                </div>
            </div>
        </div>
    );
};

export default memo(LogicIfNode);
