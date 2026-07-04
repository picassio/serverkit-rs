import { useEffect } from 'react';

const FOCUSABLE = [
    'a[href]', 'button:not([disabled])', 'textarea:not([disabled])',
    'input:not([disabled])', 'select:not([disabled])', '[tabindex]:not([tabindex="-1"])',
].join(',');

// Traps focus within `ref` while `active`, calls `onEscape` on Esc, and
// restores focus to `restoreFocusRef` (or the previously focused element) on
// deactivation.
export default function useFocusTrap(ref, { active, onEscape, restoreFocusRef } = {}) {
    useEffect(() => {
        if (!active || !ref.current) return undefined;
        const node = ref.current;
        const previouslyFocused = document.activeElement;

        const focusFirst = () => {
            const focusable = node.querySelectorAll(FOCUSABLE);
            (focusable[0] || node).focus?.();
        };
        focusFirst();

        const onKeyDown = (e) => {
            if (e.key === 'Escape') {
                onEscape?.();
                return;
            }
            if (e.key !== 'Tab') return;
            const focusable = Array.from(node.querySelectorAll(FOCUSABLE)).filter(
                (el) => el.offsetParent !== null,
            );
            if (!focusable.length) return;
            const first = focusable[0];
            const last = focusable[focusable.length - 1];
            if (e.shiftKey && document.activeElement === first) {
                e.preventDefault();
                last.focus();
            } else if (!e.shiftKey && document.activeElement === last) {
                e.preventDefault();
                first.focus();
            }
        };

        node.addEventListener('keydown', onKeyDown);
        return () => {
            node.removeEventListener('keydown', onKeyDown);
            const restoreTo = restoreFocusRef?.current || previouslyFocused;
            restoreTo?.focus?.();
        };
    }, [active, ref, onEscape, restoreFocusRef]);
}
