import React, { useCallback, useMemo, useRef, useState, useEffect } from 'react';
import {
    ReactFlow,
    ReactFlowProvider,
    useNodesState,
    useEdgesState,
    useReactFlow,
    addEdge,
    Background,
    Controls,
    MiniMap
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { Server, Database, Globe, Box, Save, FolderOpen, Plus, RefreshCw, Play, Layout, Eye, Bell, Terminal, Activity } from 'lucide-react';
import api from '../services/api';
import WorkflowListModal from '../components/workflow/WorkflowListModal';
import DeploymentProgressModal from '../components/workflow/DeploymentProgressModal';
import WorkflowExecutionHistory from '../components/workflow/WorkflowExecutionHistory';
import DockerAppNode from '../components/workflow/nodes/DockerAppNode';
import DatabaseNode from '../components/workflow/nodes/DatabaseNode';
import DomainNode from '../components/workflow/nodes/DomainNode';
import ServiceNode from '../components/workflow/nodes/ServiceNode';
import TriggerNode from '../components/workflow/nodes/TriggerNode';
import ScriptNode from '../components/workflow/nodes/ScriptNode';
import NotificationNode from '../components/workflow/nodes/NotificationNode';
import LogicIfNode from '../components/workflow/nodes/LogicIfNode';
import DockerAppConfigPanel from '../components/workflow/panels/DockerAppConfigPanel';
import DatabaseConfigPanel from '../components/workflow/panels/DatabaseConfigPanel';
import DomainConfigPanel from '../components/workflow/panels/DomainConfigPanel';
import ServiceConfigPanel from '../components/workflow/panels/ServiceConfigPanel';
import TriggerConfigPanel from '../components/workflow/panels/TriggerConfigPanel';
import ScriptConfigPanel from '../components/workflow/panels/ScriptConfigPanel';
import NotificationConfigPanel from '../components/workflow/panels/NotificationConfigPanel';
import LogicIfConfigPanel from '../components/workflow/panels/LogicIfConfigPanel';
import { isValidConnection as checkValidConnection, getConnectionError, getConnectionType } from '../utils/connectionRules';
import ConnectionEdge from '../components/workflow/ConnectionEdge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { Pill } from '../components/ds';

const initialNodes = [];
const initialEdges = [];

const nodeTypes = {
    dockerApp: DockerAppNode,
    database: DatabaseNode,
    domain: DomainNode,
    service: ServiceNode,
    trigger: TriggerNode,
    script: ScriptNode,
    notification: NotificationNode,
    logic_if: LogicIfNode
};

// Categorical node palette (minimap tints). Literal hex by design: these feed
// ReactFlow inline color props, where var() does not resolve — values are
// aligned to the redesign palette (docker brand blue kept).
const nodeColorMap = {
    dockerApp: '#2496ed',
    database: '#f5b945',
    domain: '#3ddc97',
    service: '#6d7cff',
    trigger: '#49c7f0',
    script: '#646b7a',
    notification: '#b07bf5',
    logic_if: '#f97316'
};

const edgeTypes = {
    connection: ConnectionEdge
};

let nodeId = 0;
const getId = () => `node_${nodeId++}`;

// Compact relative timestamp for the header meta (backend stores naive UTC,
// so clamp negatives to "just now" rather than showing future times).
const timeAgo = (dateStr) => {
    if (!dateStr) return null;
    const diff = Date.now() - new Date(dateStr).getTime();
    if (Number.isNaN(diff)) return null;
    const mins = Math.floor(diff / 60000);
    if (mins < 1) return 'just now';
    if (mins < 60) return `${mins}m ago`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h ago`;
    return `${Math.floor(hours / 24)}d ago`;
};

const NodePalette = ({ onAddNode }) => {
    return (
        <aside className="wf-rail">
            <div className="wf-rail__grp">Triggers</div>
            <button
                type="button"
                className="wf-rail__node"
                onClick={() => onAddNode('trigger', { label: 'Manual Trigger', triggerType: 'manual', isActive: true })}
            >
                <span className="wf-rail__pi wf-rail__pi--trigger"><Play size={13} /></span>
                <span>Manual</span>
            </button>
            <button
                type="button"
                className="wf-rail__node"
                onClick={() => onAddNode('trigger', { label: 'Scheduled Task', triggerType: 'cron', isActive: true, triggerConfig: { cron: '0 * * * *' } })}
            >
                <span className="wf-rail__pi wf-rail__pi--trigger"><Activity size={13} /></span>
                <span>Schedule (Cron)</span>
            </button>
            <button
                type="button"
                className="wf-rail__node"
                onClick={() => onAddNode('trigger', { label: 'Webhook Trigger', triggerType: 'webhook', isActive: true, triggerConfig: {} })}
            >
                <span className="wf-rail__pi wf-rail__pi--trigger"><Globe size={13} /></span>
                <span>Webhook</span>
            </button>
            <button
                type="button"
                className="wf-rail__node"
                onClick={() => onAddNode('trigger', { label: 'Event Listener', triggerType: 'event', isActive: true, triggerConfig: { eventType: 'health_check_failed' } })}
            >
                <span className="wf-rail__pi wf-rail__pi--trigger"><Eye size={13} /></span>
                <span>System Event</span>
            </button>

            <div className="wf-rail__grp">Actions</div>
            <button
                type="button"
                className="wf-rail__node"
                onClick={() => onAddNode('script', { label: 'Run Script', language: 'bash', content: '' })}
            >
                <span className="wf-rail__pi wf-rail__pi--action"><Terminal size={13} /></span>
                <span>Run Script</span>
            </button>
            <button
                type="button"
                className="wf-rail__node"
                onClick={() => onAddNode('notification', { label: 'Send Notification', channel: 'system', message: '' })}
            >
                <span className="wf-rail__pi wf-rail__pi--action"><Bell size={13} /></span>
                <span>Notification</span>
            </button>

            <div className="wf-rail__grp">Flow Control</div>
            <button
                type="button"
                className="wf-rail__node"
                onClick={() => onAddNode('logic_if', { label: 'If/Else', condition: '' })}
            >
                <span className="wf-rail__pi wf-rail__pi--logic"><Layout size={13} /></span>
                <span>Condition (If/Else)</span>
            </button>

            <div className="wf-rail__hint">
                Add a trigger, connect actions, then save and execute. Press Delete to remove selected nodes.
            </div>
        </aside>
    );
};

const WorkflowCanvas = () => {
    const reactFlowWrapper = useRef(null);
    const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
    const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);
    const [selectedNode, setSelectedNode] = useState(null);
    const [selectedEdge, setSelectedEdge] = useState(null);
    const [connectionError, setConnectionError] = useState(null);
    const { screenToFlowPosition, getViewport, setViewport, fitView } = useReactFlow();

    // Workflow state
    const [currentWorkflow, setCurrentWorkflow] = useState(null);
    const [workflowName, setWorkflowName] = useState('Untitled Workflow');
    const [isSaving, setIsSaving] = useState(false);
    const [isLoading, setIsLoading] = useState(false);
    const [showLoadModal, setShowLoadModal] = useState(false);
    const [saveMessage, setSaveMessage] = useState(null);
    const [isTogglingActive, setIsTogglingActive] = useState(false);

    // Server data state
    const [templates, setTemplates] = useState([]);
    const [allApps, setAllApps] = useState([]);
    const [appsInView, setAppsInView] = useState(new Set());

    // Deployment state
    const [isDeploying, setIsDeploying] = useState(false);
    const [showDeployModal, setShowDeployModal] = useState(false);
    const [deploymentResults, setDeploymentResults] = useState(null);

    // Automation state
    const [isExecuting, setIsExecuting] = useState(false);
    const [showHistory, setShowHistory] = useState(false);
    const [executionId, setExecutionId] = useState(null);

    const memoizedNodeTypes = useMemo(() => nodeTypes, []);
    const memoizedEdgeTypes = useMemo(() => edgeTypes, []);

    // Delete edge by ID
    const deleteEdge = useCallback((edgeId) => {
        setEdges((eds) => eds.filter((e) => e.id !== edgeId));
        setSelectedEdge(null);
    }, [setEdges]);

    // Fetch templates on mount
    useEffect(() => {
        const fetchTemplates = async () => {
            try {
                const response = await api.getTemplates();
                setTemplates(response.templates || []);
            } catch (error) {
                console.error('Failed to fetch templates:', error);
            }
        };
        fetchTemplates();
    }, []);

    // Fetch all apps for the existing apps list
    useEffect(() => {
        const fetchApps = async () => {
            try {
                const response = await api.getApps();
                setAllApps(response.apps || []);
            } catch (error) {
                console.error('Failed to fetch apps:', error);
            }
        };
        fetchApps();
    }, []);

    // Get apps not currently in the view
    const existingAppsNotInView = useMemo(() => {
        return allApps.filter(app => !appsInView.has(app.id));
    }, [allApps, appsInView]);

    // Load server overview - all infrastructure with connections
    const loadServerOverview = useCallback(async () => {
        setIsLoading(true);
        setSaveMessage(null);

        try {
            const [appsResponse, domainsResponse] = await Promise.all([
                api.getApps().catch(() => ({ apps: [] })),
                api.getDomains().catch(() => ({ domains: [] }))
            ]);

            const apps = appsResponse.apps || [];
            const domains = domainsResponse.domains || [];

            // Fetch databases for each Docker app in parallel
            const appDatabasePromises = apps
                .filter(app => app.app_type === 'docker')
                .map(app => api.getAppDatabases(app.id).catch(() => ({ databases: [] })));
            const appDatabaseResults = await Promise.all(appDatabasePromises);

            // Create a map of app_id to databases
            const appDatabasesMap = {};
            apps.filter(app => app.app_type === 'docker').forEach((app, index) => {
                appDatabasesMap[app.id] = appDatabaseResults[index]?.databases || [];
            });

            if (apps.length === 0 && domains.length === 0) {
                setSaveMessage('No infrastructure found - create an app first!');
                setTimeout(() => setSaveMessage(null), 4000);
                setIsLoading(false);
                return;
            }

            const importedNodes = [];
            const importedEdges = [];
            const appNodeIds = {};
            const domainNodeIds = {};

            // Layout configuration
            const GRID_SPACING_X = 320;
            const GRID_SPACING_Y = 180;
            const COLS = 3;
            const DOMAIN_OFFSET_Y = 150;

            // Reset node ID counter
            nodeId = 0;

            // Create app nodes
            apps.forEach((app, index) => {
                const col = index % COLS;
                const row = Math.floor(index / COLS);
                const nodeIdStr = getId();
                appNodeIds[app.id] = nodeIdStr;

                importedNodes.push({
                    id: nodeIdStr,
                    type: 'dockerApp',
                    position: {
                        x: 150 + col * GRID_SPACING_X,
                        y: 200 + row * GRID_SPACING_Y
                    },
                    data: {
                        name: app.name,
                        appId: app.id,
                        status: app.status || 'stopped',
                        port: app.port,
                        appType: app.app_type,
                        template: app.docker_image || app.app_type,
                        privateUrl: app.private_url_enabled ? `/p/${app.private_slug}` : null,
                        domains: app.domains || [],
                        isReal: true
                    }
                });
            });

            // Create domain nodes positioned above their connected apps
            const processedDomains = new Set();
            apps.forEach((app) => {
                if (app.domains && app.domains.length > 0) {
                    app.domains.forEach((domain, domIdx) => {
                        if (processedDomains.has(domain.id)) return;
                        processedDomains.add(domain.id);

                        const appNodeId = appNodeIds[app.id];
                        const appNode = importedNodes.find(n => n.id === appNodeId);
                        const domainNodeId = getId();
                        domainNodeIds[domain.id] = domainNodeId;

                        // Position domain above its app
                        importedNodes.push({
                            id: domainNodeId,
                            type: 'domain',
                            position: {
                                x: appNode.position.x + (domIdx * 80),
                                y: appNode.position.y - DOMAIN_OFFSET_Y
                            },
                            data: {
                                name: domain.name,
                                domainId: domain.id,
                                ssl: domain.ssl_enabled ? 'active' : 'none',
                                dnsStatus: 'propagated',
                                connectedAppId: app.id,
                                isReal: true
                            }
                        });

                        // Create edge from domain to app
                        importedEdges.push({
                            id: `edge_${domainNodeId}_${appNodeId}`,
                            source: domainNodeId,
                            target: appNodeId,
                            sourceHandle: 'output',
                            targetHandle: 'input',
                            type: 'connection',
                            animated: true,
                            data: {
                                sourceType: 'domain',
                                targetType: 'dockerApp',
                                connectionType: 'routes',
                                onDelete: deleteEdge
                            }
                        });
                    });
                }
            });

            // Add orphan domains (not connected to any app)
            const orphanDomains = domains.filter(d => !processedDomains.has(d.id));
            const appsRows = Math.ceil(apps.length / COLS);
            orphanDomains.forEach((domain, index) => {
                const col = index % COLS;
                const row = appsRows + 1 + Math.floor(index / COLS);
                const domainNodeId = getId();

                importedNodes.push({
                    id: domainNodeId,
                    type: 'domain',
                    position: {
                        x: 150 + col * GRID_SPACING_X,
                        y: 200 + row * GRID_SPACING_Y
                    },
                    data: {
                        name: domain.name,
                        domainId: domain.id,
                        ssl: domain.ssl_enabled ? 'active' : 'none',
                        dnsStatus: 'pending',
                        isOrphan: true,
                        isReal: true
                    }
                });
            });

            // Create database nodes below apps that have databases
            const DATABASE_OFFSET_Y = 180;
            let databaseCount = 0;
            apps.forEach((app) => {
                const databases = appDatabasesMap[app.id] || [];
                if (databases.length > 0) {
                    const appNodeId = appNodeIds[app.id];
                    const appNode = importedNodes.find(n => n.id === appNodeId);

                    databases.forEach((db, dbIdx) => {
                        const dbNodeId = getId();
                        databaseCount++;

                        // Position database below its app
                        importedNodes.push({
                            id: dbNodeId,
                            type: 'database',
                            position: {
                                x: appNode.position.x + (dbIdx * 200),
                                y: appNode.position.y + DATABASE_OFFSET_Y
                            },
                            data: {
                                name: db.database || db.name || 'Database',
                                type: db.type || 'mysql',
                                host: db.container || db.host || 'localhost',
                                port: db.port || '3306',
                                status: 'running',
                                connectedAppId: app.id,
                                containerName: db.container,
                                isReal: true
                            }
                        });

                        // Create edge from app's database handle to database node
                        importedEdges.push({
                            id: `edge_${appNodeId}_${dbNodeId}`,
                            source: appNodeId,
                            target: dbNodeId,
                            sourceHandle: 'database',
                            targetHandle: 'input',
                            type: 'connection',
                            animated: true,
                            data: {
                                sourceType: 'dockerApp',
                                targetType: 'database',
                                connectionType: 'uses',
                                onDelete: deleteEdge
                            }
                        });
                    });
                }
            });

            // Update state
            setNodes(importedNodes);
            setEdges(importedEdges);
            setAppsInView(new Set(apps.map(a => a.id)));
            setCurrentWorkflow(null);
            setWorkflowName('Server Overview');

            // Fit view after a short delay
            setTimeout(() => fitView({ padding: 0.2 }), 100);

            const dbMessage = databaseCount > 0 ? `, ${databaseCount} databases` : '';
            setSaveMessage(`Loaded ${apps.length} apps, ${domains.length} domains${dbMessage}`);
            setTimeout(() => setSaveMessage(null), 3000);

        } catch (error) {
            console.error('Failed to load server overview:', error);
            setSaveMessage('Failed to load infrastructure');
            setTimeout(() => setSaveMessage(null), 3000);
        } finally {
            setIsLoading(false);
        }
    }, [setNodes, setEdges, deleteEdge, fitView]);

    // Auto-load server overview on mount
    useEffect(() => {
        loadServerOverview();
    }, []);

    // Validate connections before allowing them
    const isValidConnection = useCallback((connection) => {
        return checkValidConnection(connection, nodes);
    }, [nodes]);

    // Save workflow
    const saveWorkflow = useCallback(async () => {
        setIsSaving(true);
        setSaveMessage(null);

        try {
            const viewport = getViewport();
            const serializableNodes = nodes.map(({ id, type, position, data }) => ({
                id, type, position,
                data: { ...data }
            }));
            const serializableEdges = edges.map(({ id, source, target, sourceHandle, targetHandle, type, animated, data }) => ({
                id, source, target, sourceHandle, targetHandle, type, animated,
                data: data ? { sourceType: data.sourceType, targetType: data.targetType, connectionType: data.connectionType } : undefined
            }));

            const workflowData = {
                name: workflowName,
                nodes: serializableNodes,
                edges: serializableEdges,
                viewport
            };

            if (currentWorkflow) {
                await api.updateWorkflow(currentWorkflow.id, workflowData);
                setSaveMessage('View saved');
            } else {
                const response = await api.createWorkflow(workflowData);
                setCurrentWorkflow(response.workflow);
                setSaveMessage('View created');
            }

            setTimeout(() => setSaveMessage(null), 3000);
        } catch (error) {
            console.error('Failed to save workflow:', error);
            setSaveMessage('Failed to save');
            setTimeout(() => setSaveMessage(null), 3000);
        } finally {
            setIsSaving(false);
        }
    }, [nodes, edges, workflowName, currentWorkflow, getViewport]);

    // Load workflow
    const loadWorkflow = useCallback((workflow) => {
        if (!workflow) return;

        setIsLoading(true);
        setShowLoadModal(false);

        try {
            const loadedNodes = (workflow.nodes || []).map((node) => ({
                ...node,
                data: { ...node.data }
            }));

            const loadedEdges = (workflow.edges || []).map((edge) => ({
                ...edge,
                data: {
                    ...(edge.data || {}),
                    onDelete: deleteEdge
                }
            }));

            const maxNodeId = loadedNodes.reduce((max, node) => {
                const numId = parseInt(node.id.replace('node_', ''), 10);
                return isNaN(numId) ? max : Math.max(max, numId);
            }, 0);
            nodeId = maxNodeId + 1;

            // Track which apps are in view
            const appIds = new Set();
            loadedNodes.forEach(node => {
                if (node.data?.appId) appIds.add(node.data.appId);
            });
            setAppsInView(appIds);

            setNodes(loadedNodes);
            setEdges(loadedEdges);
            setWorkflowName(workflow.name || 'Untitled Workflow');
            setCurrentWorkflow(workflow);

            if (workflow.viewport) {
                setTimeout(() => setViewport(workflow.viewport), 50);
            }

            setSaveMessage(`Loaded: ${workflow.name}`);
            setTimeout(() => setSaveMessage(null), 3000);
        } catch (error) {
            console.error('Failed to load workflow:', error);
            setSaveMessage('Failed to load');
            setTimeout(() => setSaveMessage(null), 3000);
        } finally {
            setIsLoading(false);
        }
    }, [setNodes, setEdges, setViewport, deleteEdge]);

    // Create new workflow
    const newWorkflow = useCallback(() => {
        setNodes([]);
        setEdges([]);
        setCurrentWorkflow(null);
        setWorkflowName('Custom View');
        setAppsInView(new Set());
        nodeId = 0;
    }, [setNodes, setEdges]);

    // Add node from template (placeholder - would create app via API)
    const addFromTemplate = useCallback((template) => {
        setSaveMessage(`To create from "${template.name}", use the Applications page`);
        setTimeout(() => setSaveMessage(null), 4000);
    }, []);

    // Add existing app to view
    const addExistingApp = useCallback((app) => {
        const newNode = {
            id: getId(),
            type: 'dockerApp',
            position: screenToFlowPosition({
                x: window.innerWidth / 2 - 90,
                y: window.innerHeight / 2 - 50
            }),
            data: {
                name: app.name,
                appId: app.id,
                status: app.status || 'stopped',
                port: app.port,
                appType: app.app_type,
                template: app.docker_image || app.app_type,
                privateUrl: app.private_url_enabled ? `/p/${app.private_slug}` : null,
                domains: app.domains || [],
                isReal: true
            }
        };

        setNodes((nds) => [...nds, newNode]);
        setAppsInView((prev) => new Set([...prev, app.id]));

        // Add edges for connected domains if they're in view
        if (app.domains && app.domains.length > 0) {
            app.domains.forEach(domain => {
                // Find if domain node exists
                const domainNode = nodes.find(n => n.data?.domainId === domain.id);
                if (domainNode) {
                    const newEdge = {
                        id: `edge_${domainNode.id}_${newNode.id}`,
                        source: domainNode.id,
                        target: newNode.id,
                        sourceHandle: 'output',
                        targetHandle: 'input',
                        type: 'connection',
                        animated: true,
                        data: {
                            sourceType: 'domain',
                            targetType: 'dockerApp',
                            connectionType: 'routes',
                            onDelete: deleteEdge
                        }
                    };
                    setEdges((eds) => [...eds, newEdge]);
                }
            });
        }
    }, [screenToFlowPosition, setNodes, setEdges, nodes, deleteEdge]);

    // Deploy workflow
    const deployWorkflow = useCallback(async () => {
        if (!currentWorkflow) {
            setSaveMessage('Save as custom view first');
            setTimeout(() => setSaveMessage(null), 3000);
            return;
        }

        if (nodes.length === 0) {
            setSaveMessage('No nodes to deploy');
            setTimeout(() => setSaveMessage(null), 3000);
            return;
        }

        setIsDeploying(true);
        setShowDeployModal(true);
        setDeploymentResults(null);

        try {
            const viewport = getViewport();
            const serializableNodes = nodes.map(({ id, type, position, data }) => ({
                id, type, position,
                data: { ...data }
            }));
            const serializableEdges = edges.map(({ id, source, target, sourceHandle, targetHandle, type, animated, data }) => ({
                id, source, target, sourceHandle, targetHandle, type, animated,
                data: data ? { sourceType: data.sourceType, targetType: data.targetType, connectionType: data.connectionType } : undefined
            }));

            await api.updateWorkflow(currentWorkflow.id, {
                name: workflowName,
                nodes: serializableNodes,
                edges: serializableEdges,
                viewport
            });

            const result = await api.deployWorkflow(currentWorkflow.id);
            setDeploymentResults(result);

            if (result.workflow && result.workflow.nodes) {
                const updatedNodes = result.workflow.nodes.map((node) => ({
                    ...node,
                    data: { ...node.data }
                }));

                const maxNodeId = updatedNodes.reduce((max, node) => {
                    const numId = parseInt(node.id.replace('node_', ''), 10);
                    return isNaN(numId) ? max : Math.max(max, numId);
                }, 0);
                nodeId = maxNodeId + 1;

                setNodes(updatedNodes);
                setCurrentWorkflow(result.workflow);
            }

        } catch (error) {
            console.error('Failed to deploy workflow:', error);
            setDeploymentResults({
                success: false,
                error: error.message || 'Deployment failed',
                results: [],
                errors: [{ error: error.message || 'Deployment failed' }]
            });
        } finally {
            setIsDeploying(false);
        }
    }, [currentWorkflow, nodes, edges, workflowName, getViewport, setNodes]);

    // Execute workflow
    const executeWorkflow = useCallback(async () => {
        if (!currentWorkflow) {
            setSaveMessage('Save workflow first');
            setTimeout(() => setSaveMessage(null), 3000);
            return;
        }

        setIsExecuting(true);
        setSaveMessage('Executing...');

        try {
            const response = await api.executeWorkflow(currentWorkflow.id);
            setExecutionId(response.execution_id);
            setSaveMessage('Execution started');
            setShowHistory(true);
            setTimeout(() => setSaveMessage(null), 3000);
        } catch (error) {
            console.error('Failed to execute workflow:', error);
            setSaveMessage('Execution failed');
            setTimeout(() => setSaveMessage(null), 3000);
        } finally {
            setIsExecuting(false);
        }
    }, [currentWorkflow]);

    // Toggle the workflow-level active/paused flag (persisted by the backend:
    // Workflow.is_active gates scheduled/webhook/event trigger runs).
    const toggleWorkflowActive = useCallback(async () => {
        if (!currentWorkflow || isTogglingActive) return;

        const next = !currentWorkflow.is_active;
        setIsTogglingActive(true);

        try {
            const response = await api.updateWorkflow(currentWorkflow.id, { is_active: next });
            setCurrentWorkflow(response.workflow || { ...currentWorkflow, is_active: next });
            setSaveMessage(next ? 'Workflow activated' : 'Workflow paused');
        } catch (error) {
            console.error('Failed to toggle workflow active state:', error);
            setSaveMessage('Failed to update workflow');
        } finally {
            setIsTogglingActive(false);
            setTimeout(() => setSaveMessage(null), 3000);
        }
    }, [currentWorkflow, isTogglingActive]);

    // Mono meta line under the workflow name (real payload fields only).
    const headMeta = useMemo(() => {
        const parts = [];
        if (currentWorkflow) parts.push(currentWorkflow.trigger_type || 'manual');
        parts.push(`${nodes.length} nodes`);
        parts.push(`${edges.length} edges`);
        if (currentWorkflow) {
            const ran = timeAgo(currentWorkflow.last_run_at);
            parts.push(ran ? `last run ${ran}` : 'never run');
            if (currentWorkflow.last_status) parts.push(currentWorkflow.last_status);
        } else {
            parts.push('unsaved view');
        }
        return parts.join(' · ');
    }, [currentWorkflow, nodes.length, edges.length]);

    const onConnect = useCallback(
        (params) => {
            if (checkValidConnection(params, nodes)) {
                const sourceNode = nodes.find(n => n.id === params.source);
                const targetNode = nodes.find(n => n.id === params.target);
                const connectionType = getConnectionType(sourceNode?.type, targetNode?.type);

                const newEdge = {
                    ...params,
                    type: 'connection',
                    animated: true,
                    data: {
                        sourceType: sourceNode?.type,
                        targetType: targetNode?.type,
                        connectionType,
                        onDelete: deleteEdge
                    }
                };
                setEdges((eds) => addEdge(newEdge, eds));
                setConnectionError(null);
            }
        },
        [setEdges, nodes, deleteEdge]
    );

    const onConnectStart = useCallback(() => {
        setConnectionError(null);
    }, []);

    const onConnectEnd = useCallback((event, connectionState) => {
        if (connectionState.isValid === false && connectionState.toNode) {
            const error = getConnectionError({
                source: connectionState.fromNode?.id,
                target: connectionState.toNode?.id,
                sourceHandle: connectionState.fromHandle?.id,
                targetHandle: connectionState.toHandle?.id
            }, nodes);
            if (error) {
                setConnectionError(error);
                setTimeout(() => setConnectionError(null), 3000);
            }
        }
    }, [nodes]);

    const addNode = useCallback((nodeType, defaultData = {}) => {
        const newNode = {
            id: getId(),
            type: nodeType,
            position: screenToFlowPosition({
                x: window.innerWidth / 2 - 90,
                y: window.innerHeight / 2 - 50
            }),
            data: defaultData
        };
        setNodes((nds) => [...nds, newNode]);
    }, [screenToFlowPosition, setNodes]);

    const getNodeColor = useCallback((node) => {
        return nodeColorMap[node.type] || '#6d7cff';
    }, []);

    const handleNodeClick = useCallback((event, node) => {
        setSelectedNode(node);
        setSelectedEdge(null);
    }, []);

    const handleEdgeClick = useCallback((event, edge) => {
        setSelectedEdge(edge);
        setSelectedNode(null);
    }, []);

    const handlePaneClick = useCallback(() => {
        setSelectedNode(null);
        setSelectedEdge(null);
    }, []);

    const handleDeleteNode = useCallback(() => {
        if (!selectedNode) return;
        setNodes((nds) => nds.filter((n) => n.id !== selectedNode.id));
        setEdges((eds) => eds.filter((e) => e.source !== selectedNode.id && e.target !== selectedNode.id));
        setSelectedNode(null);
    }, [selectedNode, setNodes, setEdges]);

    const handleKeyDown = useCallback((event) => {
        // Don't delete when typing in an input/textarea
        if (event.target.tagName === 'INPUT' || event.target.tagName === 'TEXTAREA' || event.target.tagName === 'SELECT') return;

        if (event.key === 'Delete' || event.key === 'Backspace') {
            if (selectedEdge) {
                deleteEdge(selectedEdge.id);
            } else if (selectedNode) {
                handleDeleteNode();
            }
        }
    }, [selectedEdge, selectedNode, deleteEdge, handleDeleteNode]);

    React.useEffect(() => {
        document.addEventListener('keydown', handleKeyDown);
        return () => document.removeEventListener('keydown', handleKeyDown);
    }, [handleKeyDown]);

    const handlePanelClose = useCallback(() => {
        setSelectedNode(null);
    }, []);

    const handleNodeDataChange = useCallback((newData) => {
        if (!selectedNode) return;

        setNodes((nds) =>
            nds.map((node) =>
                node.id === selectedNode.id
                    ? { ...node, data: newData }
                    : node
            )
        );

        setSelectedNode((prev) => prev ? { ...prev, data: newData } : null);
    }, [selectedNode, setNodes]);

    const renderConfigPanel = () => {
        if (!selectedNode) return null;

        const panelProps = {
            node: selectedNode,
            onChange: handleNodeDataChange,
            onClose: handlePanelClose,
            onDelete: handleDeleteNode
        };

        switch (selectedNode.type) {
            case 'dockerApp':
                return <DockerAppConfigPanel {...panelProps} />;
            case 'database':
                return <DatabaseConfigPanel {...panelProps} />;
            case 'domain':
                return <DomainConfigPanel {...panelProps} />;
            case 'service':
                return <ServiceConfigPanel {...panelProps} />;
            case 'trigger':
                return <TriggerConfigPanel {...panelProps} />;
            case 'script':
                return <ScriptConfigPanel {...panelProps} />;
            case 'notification':
                return <NotificationConfigPanel {...panelProps} />;
            case 'logic_if':
                return <LogicIfConfigPanel {...panelProps} />;
            default:
                return null;
        }
    };

    return (
        <div className="workflow-canvas" ref={reactFlowWrapper}>
            <div className="wf-head">
                <div className="wf-head__main">
                    <div className="wf-head__titlerow">
                        <Input
                            type="text"
                            className="workflow-name-input"
                            value={workflowName}
                            onChange={(e) => setWorkflowName(e.target.value)}
                            placeholder="View name..."
                        />
                        {currentWorkflow && (
                            <Pill kind={currentWorkflow.is_active ? 'green' : 'gray'}>
                                {currentWorkflow.is_active ? 'active' : 'paused'}
                            </Pill>
                        )}
                    </div>
                    <div className="wf-head__meta">{headMeta}</div>
                </div>
                <div className="wf-head__actions">
                    {currentWorkflow && (
                        <label className="wf-active-toggle" title="Enable automated trigger runs for this workflow">
                            <span>Active</span>
                            <Switch
                                checked={!!currentWorkflow.is_active}
                                onCheckedChange={toggleWorkflowActive}
                                disabled={isTogglingActive}
                            />
                        </label>
                    )}
                    <Button
                        variant="ghost"
                        size="sm"
                        className="toolbar-btn"
                        onClick={newWorkflow}
                        title="New workflow"
                    >
                        <Plus size={15} />
                        <span>New</span>
                    </Button>
                    <Button
                        variant="ghost"
                        size="sm"
                        className="toolbar-btn"
                        onClick={() => setShowLoadModal(true)}
                        title="Load saved workflow"
                    >
                        <FolderOpen size={15} />
                        <span>Load</span>
                    </Button>
                    <div className="toolbar-divider" />
                    <Button
                        variant="ghost"
                        size="sm"
                        className="toolbar-btn toolbar-btn-execute"
                        onClick={executeWorkflow}
                        disabled={isExecuting || !currentWorkflow}
                        title="Execute workflow"
                    >
                        <Play size={15} />
                        <span>{isExecuting ? 'Running...' : 'Execute'}</span>
                    </Button>
                    <Button
                        variant="ghost"
                        size="sm"
                        className="toolbar-btn"
                        onClick={() => setShowHistory(true)}
                        disabled={!currentWorkflow}
                        title="Execution history"
                    >
                        <Activity size={15} />
                        <span>History</span>
                    </Button>
                    <div className="toolbar-divider" />
                    <Button
                        size="sm"
                        className="toolbar-btn toolbar-btn-primary"
                        onClick={saveWorkflow}
                        disabled={isSaving}
                        title="Save workflow"
                    >
                        <Save size={15} />
                        <span>{isSaving ? 'Saving...' : 'Save'}</span>
                    </Button>
                </div>
                {saveMessage && (
                    <div className="toolbar-message">{saveMessage}</div>
                )}
            </div>
            <div className="wf-body">
                <NodePalette onAddNode={addNode} />
                <div className="wf-stage">
                    {connectionError && (
                        <div className="connection-error-toast">
                            {connectionError}
                        </div>
                    )}
                    <ReactFlow
                        nodes={nodes}
                        edges={edges}
                        nodeTypes={memoizedNodeTypes}
                        edgeTypes={memoizedEdgeTypes}
                        onNodesChange={onNodesChange}
                        onEdgesChange={onEdgesChange}
                        onConnect={onConnect}
                        onConnectStart={onConnectStart}
                        onConnectEnd={onConnectEnd}
                        isValidConnection={isValidConnection}
                        onNodeClick={handleNodeClick}
                        onEdgeClick={handleEdgeClick}
                        onPaneClick={handlePaneClick}
                        fitView
                        panOnScroll
                        selectionOnDrag
                        panOnDrag={[1, 2]}
                        selectNodesOnDrag={false}
                        defaultEdgeOptions={{
                            type: 'connection',
                            animated: true
                        }}
                    >
                        <Background
                            variant="dots"
                            gap={22}
                            size={1}
                            color="var(--border)"
                        />
                        <Controls
                            showZoom={true}
                            showFitView={true}
                            showInteractive={false}
                        />
                        <MiniMap nodeColor={getNodeColor} />
                    </ReactFlow>
                    {renderConfigPanel()}
                </div>
            </div>
            {showLoadModal && (
                <WorkflowListModal
                    onLoad={loadWorkflow}
                    onClose={() => setShowLoadModal(false)}
                />
            )}
            {showDeployModal && (
                <DeploymentProgressModal
                    isDeploying={isDeploying}
                    results={deploymentResults}
                    nodes={nodes}
                    onClose={() => setShowDeployModal(false)}
                />
            )}
            {showHistory && (
                <WorkflowExecutionHistory
                    workflowId={currentWorkflow?.id}
                    onClose={() => setShowHistory(false)}
                />
            )}
        </div>
    );
};

const WorkflowBuilder = () => {
    return (
        <div className="workflow-page fullscreen">
            <ReactFlowProvider>
                <WorkflowCanvas />
            </ReactFlowProvider>
        </div>
    );
};

export default WorkflowBuilder;
