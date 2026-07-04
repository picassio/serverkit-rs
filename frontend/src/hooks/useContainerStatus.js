import { useState, useEffect, useCallback, useRef } from 'react';
import socketService from '../services/socket';
import api from '../services/api';

// Default REST poll fallback interval in ms (10 seconds).
const DEFAULT_POLL_INTERVAL = 10000;

/**
 * Subscribe to the aggregated container status for a single application.
 *
 * Prefers the real-time 'container_status' socket channel; falls back to a
 * periodic REST poll when the socket isn't connected. Returns the aggregated
 * status dict ({status, total, healthy, reasons, containers, ...}) plus a
 * manual `refetch` and a `connected` flag.
 *
 * @param {number|string} appId
 * @param {object}  [opts]
 * @param {boolean} [opts.useWebSocket=true]
 * @param {number}  [opts.pollInterval=10000]
 */
export function useContainerStatus(appId, opts = {}) {
    const { useWebSocket = true, pollInterval = DEFAULT_POLL_INTERVAL } = opts;

    const [status, setStatus] = useState(null);
    const [loading, setLoading] = useState(true);
    const [connected, setConnected] = useState(false);
    const pollRef = useRef(null);

    const refetch = useCallback(async () => {
        if (appId === null || appId === undefined) return;
        try {
            const data = await api.getAppContainerStatus(appId);
            setStatus(data);
        } catch {
            // Leave the last-known status in place on a transient error.
        } finally {
            setLoading(false);
        }
    }, [appId]);

    const startPolling = useCallback(() => {
        if (pollRef.current || pollInterval <= 0) return;
        pollRef.current = setInterval(refetch, pollInterval);
    }, [refetch, pollInterval]);

    const stopPolling = useCallback(() => {
        if (pollRef.current) {
            clearInterval(pollRef.current);
            pollRef.current = null;
        }
    }, []);

    useEffect(() => {
        if (appId === null || appId === undefined) return undefined;

        // Initial fetch so the pill has a value immediately.
        refetch();

        if (!useWebSocket) {
            startPolling();
            return () => stopPolling();
        }

        socketService.connect();

        // The status channel isn't a first-class method on the socket service,
        // so attach to the raw socket and emit the subscribe directly. Both are
        // guarded for the not-yet-connected case.
        const subscribe = () => {
            setConnected(true);
            stopPolling();
            socketService.socket?.emit('subscribe_container_status');
        };

        // Each tick the server pushes only the apps that changed; reconcile by id.
        const onStatus = (payload) => {
            const statuses = payload?.statuses || [];
            const mine = statuses.find((s) => String(s.app_id) === String(appId));
            if (mine) {
                setStatus((prev) => ({ ...(prev || {}), ...mine, app_id: mine.app_id }));
                setLoading(false);
            }
        };

        if (socketService.socket?.connected) subscribe();
        const unsubConnect = socketService.on('connected', subscribe);
        const unsubDisconnect = socketService.on('disconnected', () => {
            setConnected(false);
            startPolling();
        });
        socketService.socket?.on('container_status', onStatus);

        // Give the socket a moment to connect before falling back to polling.
        const pollTimeout = setTimeout(() => {
            if (!socketService.socket?.connected) startPolling();
        }, 3000);

        return () => {
            clearTimeout(pollTimeout);
            unsubConnect();
            unsubDisconnect();
            socketService.socket?.off('container_status', onStatus);
            socketService.socket?.emit('unsubscribe_container_status');
            stopPolling();
        };
    }, [appId, useWebSocket, refetch, startPolling, stopPolling]);

    return { status, loading, connected, refetch };
}

export default useContainerStatus;
