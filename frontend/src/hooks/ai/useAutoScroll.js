import { useCallback, useEffect, useRef, useState } from 'react';

// Keeps a scroll container pinned to the bottom as content streams in, unless
// the user has scrolled up (then we stop auto-scrolling and expose `isPinned`
// so the UI can show a "jump to latest" affordance). `deps` triggers a
// re-scroll when content changes.
export default function useAutoScroll(deps) {
    const ref = useRef(null);
    const [isPinned, setIsPinned] = useState(true);

    const checkPinned = useCallback(() => {
        const el = ref.current;
        if (!el) return;
        const distance = el.scrollHeight - el.scrollTop - el.clientHeight;
        setIsPinned(distance < 48);
    }, []);

    const scrollToBottom = useCallback(() => {
        const el = ref.current;
        if (el) el.scrollTop = el.scrollHeight;
        setIsPinned(true);
    }, []);

    useEffect(() => {
        if (isPinned) {
            const el = ref.current;
            if (el) el.scrollTop = el.scrollHeight;
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [deps]);

    return { ref, isPinned, checkPinned, scrollToBottom };
}
