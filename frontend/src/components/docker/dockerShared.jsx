import { Trash2 } from 'lucide-react';

// Icon Action button used by remaining (legacy) tabs (Images, Networks, Volumes).
// Containers tab now renders cards with full action buttons inline.
export const IconAction = ({ title, onClick, color, children, disabled }) => (
    <button type="button"
        className="docker-icon-action"
        title={title}
        onClick={onClick}
        disabled={disabled}
        style={color ? { color } : {}}
    >
        {children}
    </button>
);

export const TrashIcon = () => <Trash2 size={14} />;

export const ContainerResourceBars = ({ stats, muted = false }) => (
    <div className={`dx-mini-resources ${muted || !stats.available ? 'is-muted' : ''}`}>
        <div className="dx-mini-resource">
            <span>CPU</span>
            <div className="dx-res-track">
                <div className="dx-res-fill cpu" style={{ width: `${stats.available ? Math.min(stats.cpu, 100) : 0}%` }} />
            </div>
            <strong>{stats.available ? `${stats.cpu.toFixed(1)}%` : '--'}</strong>
        </div>
        <div className="dx-mini-resource">
            <span>RAM</span>
            <div className="dx-res-track">
                <div className="dx-res-fill mem" style={{ width: `${stats.available ? Math.min(stats.memory, 100) : 0}%` }} />
            </div>
            <strong>{stats.available ? `${stats.memory.toFixed(1)}%` : '--'}</strong>
        </div>
    </div>
);

// Download Icon for Compose Pull
export const DownloadIcon = () => (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
        <polyline points="7 10 12 15 17 10"/>
        <line x1="12" y1="15" x2="12" y2="3"/>
    </svg>
);
