import { cn } from '@/lib/utils';
import { gaugeColor } from './utils';

// Thin inline meter bar. By default the fill is threshold-colored
// (red > 75, amber > 50, else accent); pass `color` to override.
export function Gauge({ value = 0, color, className }) {
    const pct = Math.max(0, Math.min(100, value));
    return (
        <div className={cn('sk-gauge', className)}>
            <span className="sk-gauge__fill" style={{ width: `${pct}%`, background: color || gaugeColor(pct) }} />
        </div>
    );
}

export default Gauge;
