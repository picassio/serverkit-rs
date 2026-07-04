import { useState, useEffect, useCallback } from 'react';
import api from '../services/api';
import { getServiceType, getStatusConfig } from '../utils/serviceTypes';

export function useService(id) {
    const [service, setService] = useState(null);
    const [deployConfig, setDeployConfig] = useState(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    const load = useCallback(async () => {
        if (!id) return;
        try {
            setLoading(true);
            setError(null);

            const appRes = await api.getApp(id);
            const app = appRes.app;

            // Fetch deploy config in parallel (non-blocking)
            let dc = null;
            try {
                const dcRes = await api.getDeployConfig(id);
                if (dcRes.configured) {
                    dc = dcRes.config;
                }
            } catch {
                // Deploy config may not exist for all apps
            }

            const typeInfo = getServiceType(app.app_type);
            const statusInfo = getStatusConfig(app.status);

            setService({
                ...app,
                typeInfo,
                statusInfo,
                isPython: ['flask', 'django'].includes(app.app_type),
                isDocker: app.app_type === 'docker',
                isRunning: app.status === 'running',
            });
            setDeployConfig(dc);
        } catch (err) {
            setError(err.message || 'Failed to load service');
        } finally {
            setLoading(false);
        }
    }, [id]);

    useEffect(() => {
        load();
    }, [load]);

    const performAction = useCallback(async (action) => {
        if (!id) return;
        if (action === 'start') await api.startApp(id);
        else if (action === 'stop') await api.stopApp(id);
        else if (action === 'restart') await api.restartApp(id);
        await load();
    }, [id, load]);

    const deleteService = useCallback(async () => {
        if (!id) return;
        await api.deleteApp(id);
    }, [id]);

    return {
        service,
        deployConfig,
        loading,
        error,
        reload: load,
        performAction,
        deleteService,
    };
}

export default useService;
