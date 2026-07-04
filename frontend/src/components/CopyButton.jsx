import { useState, useCallback } from 'react';
import { Copy, Check } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

/**
 * Copy-to-clipboard button with built-in success feedback.
 *
 *   <CopyButton value={apiKey} label="Copy API key" />
 *   <CopyButton value={command} size="sm" variant="outline">Copy</CopyButton>
 */
export function CopyButton({
    value,
    label = 'Copy',
    copiedLabel = 'Copied!',
    size = 'icon',
    variant = 'ghost',
    className,
    children,
    onCopy,
    timeout = 2000,
}) {
    const [copied, setCopied] = useState(false);

    const handleCopy = useCallback(async () => {
        try {
            await navigator.clipboard.writeText(String(value));
            setCopied(true);
            onCopy?.(value);
            setTimeout(() => setCopied(false), timeout);
        } catch {
            // Ignore copy errors; caller can handle via onCopy if needed.
        }
    }, [value, onCopy, timeout]);

    return (
        <Button
            type="button"
            size={size}
            variant={variant}
            className={cn('copy-button', copied && 'is-copied', className)}
            onClick={handleCopy}
            aria-label={copied ? copiedLabel : label}
            title={copied ? copiedLabel : label}
        >
            {copied ? <Check size={14} /> : <Copy size={14} />}
            {children && <span className="copy-button__label">{copied ? copiedLabel : children}</span>}
        </Button>
    );
}

export default CopyButton;
