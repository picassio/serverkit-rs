import { useEffect } from 'react';
import { useOutletContext } from 'react-router-dom';

// Publish a page's top-bar actions into the enclosing TabGroupLayout header.
// Pages in a tab group render no PageTopbar of their own, so the shared layout
// shows their actions instead.
//
// Pass a RENDER THUNK that returns the action node — `() => (<Button…/>)` — not
// the node itself. The thunk is invoked inside the effect (after render), so it
// can safely reference handlers declared anywhere in the component without
// hitting the temporal dead zone. (A raw node is still accepted for
// convenience.) `deps` are the reactive values the node reads (e.g.
// [loading, isAdmin]); the node/thunk itself is intentionally not a dependency.
// No-op when rendered outside a tab group (e.g. a page reused as a plain route).
export function useTopbarActions(actions, deps = []) {
    const ctx = useOutletContext();
    const setTopbarActions = ctx?.setTopbarActions;

    useEffect(() => {
        if (!setTopbarActions) return undefined;
        setTopbarActions(typeof actions === 'function' ? actions() : actions);
        return () => setTopbarActions(null);
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [setTopbarActions, ...deps]);
}

export default useTopbarActions;
