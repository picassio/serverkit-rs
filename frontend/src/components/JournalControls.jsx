import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';

const PRIORITY_OPTIONS = [
    { value: '', label: 'All' },
    { value: '0', label: 'Emergency' },
    { value: '1', label: 'Alert' },
    { value: '2', label: 'Critical' },
    { value: '3', label: 'Error' },
    { value: '4', label: 'Warning' },
    { value: '5', label: 'Notice' },
    { value: '6', label: 'Info' },
    { value: '7', label: 'Debug' },
];

export function JournalControls({
    unit = '',
    onUnitChange,
    unitLabel = 'Service/Unit',
    unitPlaceholder = 'All services',
    quickUnits = [],
    showQuickUnits = true,
    lineCount,
    onLineCountChange,
    lineCountOptions = [50, 100, 200, 500],
    priority,
    onPriorityChange,
    showPriority = true,
    loading = false,
    onLoad,
    loadLabel = 'Load Logs',
}) {
    return (
        <div className="journal-controls">
            <div className="control-group">
                <label>{unitLabel}</label>
                <div className="input-with-suggestions">
                    <Input
                        type="text"
                        value={unit}
                        onChange={(e) => onUnitChange?.(e.target.value)}
                        placeholder={unitPlaceholder}
                    />
                    {showQuickUnits && quickUnits.length > 0 && (
                        <div className="quick-units">
                            {quickUnits.map(u => (
                                <button type="button"
                                    key={u}
                                    className={`unit-chip ${unit === u ? 'active' : ''}`}
                                    onClick={() => onUnitChange?.(unit === u ? '' : u)}
                                >
                                    {u}
                                </button>
                            ))}
                        </div>
                    )}
                </div>
            </div>

            {onLineCountChange && (
                <div className="control-group">
                    <label>Lines</label>
                    <select value={lineCount} onChange={(e) => onLineCountChange(parseInt(e.target.value, 10))}>
                        {lineCountOptions.map(n => (
                            <option key={n} value={n}>{n}</option>
                        ))}
                    </select>
                </div>
            )}

            {showPriority && onPriorityChange && (
                <div className="control-group">
                    <label>Priority</label>
                    <select value={priority ?? ''} onChange={(e) => onPriorityChange(e.target.value)}>
                        {PRIORITY_OPTIONS.map(o => (
                            <option key={o.value} value={o.value}>{o.label}</option>
                        ))}
                    </select>
                </div>
            )}

            {onLoad && (
                <Button onClick={onLoad} disabled={loading}>
                    {loading ? 'Loading...' : loadLabel}
                </Button>
            )}
        </div>
    );
}

export default JournalControls;
