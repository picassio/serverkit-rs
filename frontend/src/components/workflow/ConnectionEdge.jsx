import { memo } from 'react';
import { BaseEdge, EdgeLabelRenderer, getSmoothStepPath } from '@xyflow/react';
import { X } from 'lucide-react';
import { connectionLabels, getConnectionType } from '../../utils/connectionRules';

// Categorical edge tints aligned to the redesign palette. Literal hex by
// design: SVG stroke is set via ReactFlow inline style props where var()
// does not resolve (docker brand blue kept).
const edgeColors = {
    'domain-dockerApp': '#3ddc97',
    'domain-service': '#3ddc97',
    'dockerApp-dockerApp': '#2496ed',
    'dockerApp-database': '#f5b945',
    'service-dockerApp': '#6d7cff',
    'service-database': '#f5b945',
    'service-service': '#6d7cff'
};

const ConnectionEdge = ({
    id,
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
    source,
    target,
    sourceHandleId,
    selected,
    data,
    markerEnd
}) => {
    const [edgePath, labelX, labelY] = getSmoothStepPath({
        sourceX,
        sourceY,
        sourcePosition,
        targetX,
        targetY,
        targetPosition,
        borderRadius: 8
    });

    // Get connection type from data or derive from source/target
    const connectionType = data?.connectionType || getConnectionType(
        data?.sourceType || 'dockerApp',
        data?.targetType || 'dockerApp'
    );

    const label = connectionLabels[connectionType] || 'Connected';
    const color = edgeColors[connectionType] || '#6d7cff';

    return (
        <>
            <BaseEdge
                id={id}
                path={edgePath}
                markerEnd={markerEnd}
                style={{
                    stroke: selected ? '#8b93ff' : color,
                    strokeWidth: selected ? 2 : 1.5,
                    filter: selected ? 'drop-shadow(0 0 4px rgba(109, 124, 255, 0.55))' : 'none'
                }}
            />
            <EdgeLabelRenderer>
                <div
                    className={`edge-label ${selected ? 'edge-label-selected' : ''}`}
                    style={{
                        position: 'absolute',
                        transform: `translate(-50%, -50%) translate(${labelX}px,${labelY}px)`,
                        pointerEvents: 'all',
                        '--edge-c': color
                    }}
                >
                    <span className="edge-label-text">{label}</span>
                    {selected && data?.onDelete && (
                        <button type="button"
                            className="edge-delete-btn"
                            onClick={(e) => {
                                e.stopPropagation();
                                data.onDelete(id);
                            }}
                        >
                            <X size={12} />
                        </button>
                    )}
                </div>
            </EdgeLabelRenderer>
        </>
    );
};

export default memo(ConnectionEdge);
