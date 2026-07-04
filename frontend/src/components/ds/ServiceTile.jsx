import { cn } from '@/lib/utils';
import { svcGrad, initials } from './utils';

// Gradient initial avatar. Defaults to a deterministic gradient + first letter
// derived from `name`; pass `gradient` and/or `label` to override.
export function ServiceTile({ name = '', size = 38, label, gradient, className, style, ...props }) {
    return (
        <span
            className={cn('sk-tile', className)}
            style={{
                width: size,
                height: size,
                fontSize: Math.round(size * 0.4),
                background: gradient || svcGrad(name),
                ...style,
            }}
            {...props}
        >
            {label || initials(name)}
        </span>
    );
}

export default ServiceTile;
