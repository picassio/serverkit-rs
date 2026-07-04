import { createContext, useContext, useCallback, useRef, useState } from 'react';
import ConfirmDialog from '../components/ConfirmDialog';

// App-level confirm dialog. One <ConfirmDialog> is rendered here; pages call
// `confirm({...})` (via useConfirm) and await the boolean result instead of
// each rendering their own dialog. See useConfirm for the back-compat shim that
// lets un-migrated pages keep working.

const ConfirmContext = createContext(null);

export function ConfirmProvider({ children }) {
  const [state, setState] = useState({ isOpen: false });
  const resolver = useRef(null);

  const confirm = useCallback((options = {}) => {
    return new Promise((resolve) => {
      resolver.current = resolve;
      setState({ ...options, isOpen: true });
    });
  }, []);

  const settle = useCallback((result) => {
    if (resolver.current) {
      resolver.current(result);
      resolver.current = null;
    }
    setState((prev) => ({ ...prev, isOpen: false }));
  }, []);

  return (
    <ConfirmContext.Provider value={confirm}>
      {children}
      <ConfirmDialog
        isOpen={state.isOpen}
        title={state.title}
        message={state.message}
        details={state.details}
        confirmText={state.confirmText}
        cancelText={state.cancelText}
        variant={state.variant}
        requireConfirmation={state.requireConfirmation}
        confirmationPlaceholder={state.confirmationPlaceholder}
        onConfirm={() => settle(true)}
        onCancel={() => settle(false)}
      />
    </ConfirmContext.Provider>
  );
}

export function useConfirmContext() {
  return useContext(ConfirmContext);
}

export default ConfirmContext;
