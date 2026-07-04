import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

/**
 * Accessible icon-only button. Always provide a descriptive label; it is used
 * for the aria-label and optional title.
 *
 *   <IconButton icon={<RefreshCw size={16} />} label="Refresh" onClick={load} />
 */
export function IconButton({
    icon,
    label,
    onClick,
    variant = 'ghost',
    size = 'sm',
    className,
    title,
    disabled,
    ...props
}) {
    return (
        <Button
            type="button"
            variant={variant}
            size="icon"
            className={cn('icon-button', size === 'sm' && 'btn-sm', className)}
            onClick={onClick}
            disabled={disabled}
            aria-label={label}
            title={title || label}
            {...props}
        >
            {icon}
        </Button>
    );
}

export default IconButton;
