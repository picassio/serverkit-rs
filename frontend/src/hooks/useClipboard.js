import { useCallback, useRef, useState } from 'react';
import { useToast } from '@/contexts/ToastContext';
import { copyToClipboard } from '@/utils/clipboard';

// Copy-to-clipboard hook with toast feedback and a transient `copied` flag.
//
//   const { copy, copied } = useClipboard();
//   <button onClick={() => copy(apiKey)}>Copy</button>
//
//   const { copy } = useClipboard({ successMessage: 'Token copied' });
//   copy(token);
//   copy(value, 'Custom one-off message');   // per-call override
//
// `copied` flips true for `resetDelay` ms after a successful copy so the
// trigger can show a check icon without each caller wiring its own timer.
export function useClipboard({
    successMessage = 'Copied to clipboard',
    errorMessage = 'Failed to copy',
    resetDelay = 2000,
} = {}) {
    const toast = useToast();
    const [copied, setCopied] = useState(false);
    const timer = useRef(null);

    const copy = useCallback(
        async (text, message) => {
            const ok = await copyToClipboard(text);
            if (ok) {
                setCopied(true);
                if (message !== null) toast.success(message ?? successMessage);
                if (timer.current) clearTimeout(timer.current);
                timer.current = setTimeout(() => setCopied(false), resetDelay);
            } else {
                toast.error(errorMessage);
            }
            return ok;
        },
        [toast, successMessage, errorMessage, resetDelay]
    );

    return { copy, copied };
}

export default useClipboard;
