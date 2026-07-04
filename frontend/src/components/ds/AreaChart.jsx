import * as React from 'react';
import { cn } from '@/lib/utils';

// Smooth multi-series area chart with gradient fills. Theme-token colors by
// default so it tracks dark/light + custom accent.
// series: a single number[] or an array of number[] (one per series).
export function AreaChart({
    series = [],
    height = 160,
    colors = ['var(--accent-bright)', 'var(--green)', 'var(--cyan)'],
    grid = true,
    className,
}) {
    const data = series.length && Array.isArray(series[0]) ? series : [series];
    const all = data.flat();
    const uid = React.useId().replace(/:/g, '');
    if (!all.length) return null;

    const w = 100;
    const h = 100;
    const max = Math.max(...all) * 1.15;
    const min = Math.min(...all) * 0.6;
    const span = max - min || 1;
    const toX = (i, len) => (i / (len - 1 || 1)) * w;
    const toY = (v) => h - ((v - min) / span) * h;

    const linePath = (s) => s.map((v, i) => `${i ? 'L' : 'M'}${toX(i, s.length).toFixed(2)} ${toY(v).toFixed(2)}`).join(' ');
    const areaPath = (s) => `${linePath(s)} L ${w} ${h} L 0 ${h} Z`;

    return (
        <svg
            className={cn('sk-area', className)}
            viewBox={`0 0 ${w} ${h}`}
            preserveAspectRatio="none"
            style={{ height }}
            aria-hidden="true"
        >
            <defs>
                {colors.map((c, i) => (
                    <linearGradient key={i} id={`sk-area-${uid}-${i}`} x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stopColor={c} stopOpacity="0.28" />
                        <stop offset="100%" stopColor={c} stopOpacity="0" />
                    </linearGradient>
                ))}
            </defs>
            {grid &&
                [0, 25, 50, 75, 100].map((y) => (
                    <line key={y} x1="0" y1={y} x2={w} y2={y} stroke="var(--border-soft)" strokeWidth="0.4" vectorEffect="non-scaling-stroke" />
                ))}
            {data.map((s, i) => (
                <path key={`a${i}`} d={areaPath(s)} fill={`url(#sk-area-${uid}-${i % colors.length})`} />
            ))}
            {data.map((s, i) => (
                <path
                    key={`l${i}`}
                    d={linePath(s)}
                    fill="none"
                    stroke={colors[i % colors.length]}
                    strokeWidth="1.6"
                    vectorEffect="non-scaling-stroke"
                    strokeLinejoin="round"
                    strokeLinecap="round"
                />
            ))}
        </svg>
    );
}

export default AreaChart;
