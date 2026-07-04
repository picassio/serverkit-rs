import { createContext, useContext, useCallback } from 'react';
import { toast as sonner } from 'sonner';

const ToastContext = createContext(null);

export function useToast() {
  const context = useContext(ToastContext);
  if (!context) throw new Error('useToast must be used within a ToastProvider');
  return context;
}

// Callers pass either a numeric duration (`toast.info(msg, 4000)`) or a full sonner
// options object (`toast.info(msg, { duration: 4000 })`). Normalize both so an
// options object is passed through instead of being double-wrapped — the latter
// silently dropped custom durations across the app.
function toastOptions(opts) {
  if (opts == null) return undefined;
  return typeof opts === 'number' ? { duration: opts } : opts;
}

export function ToastProvider({ children }) {
  const success = useCallback((message, opts) => sonner.success(message, toastOptions(opts)), []);
  const error = useCallback((message, opts) => sonner.error(message, toastOptions(opts)), []);
  const warning = useCallback((message, opts) => sonner.warning(message, toastOptions(opts)), []);
  const info = useCallback((message, opts) => sonner.info(message, toastOptions(opts)), []);

  return (
    <ToastContext.Provider value={{ success, error, warning, info, toasts: [], addToast: sonner, removeToast: () => {} }}>
      {children}
    </ToastContext.Provider>
  );
}

export default ToastContext;
