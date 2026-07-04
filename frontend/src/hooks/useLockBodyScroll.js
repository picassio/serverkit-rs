import { useEffect } from 'react';

// Lock body scroll while `locked` is true. Ref-counted so nested locks (e.g. a
// modal opening over an already-locked drawer) compose correctly — the body
// overflow is only restored once the LAST active lock is released.
//
//   useLockBodyScroll(isOpen);
//
// Replaces the hand-rolled `document.body.style.overflow` toggles that were
// duplicated across DashboardLayout, PreviewDrawer, and ChatDrawer.

let lockCount = 0;
let originalOverflow = '';

export function useLockBodyScroll(locked = true) {
  useEffect(() => {
    if (!locked || typeof document === 'undefined') return undefined;

    if (lockCount === 0) {
      originalOverflow = document.body.style.overflow;
      document.body.style.overflow = 'hidden';
    }
    lockCount += 1;

    return () => {
      lockCount -= 1;
      if (lockCount === 0) {
        document.body.style.overflow = originalOverflow;
      }
    };
  }, [locked]);
}

export default useLockBodyScroll;
