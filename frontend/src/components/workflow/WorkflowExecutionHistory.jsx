import { useState, useEffect, useCallback } from 'react';
import { Clock, Loader2, FileText, Activity } from 'lucide-react';
import api from '../../services/api';
import Modal from '../Modal';
import { Pill } from '../ds';

const statusKind = {
    success: 'green',
    failed: 'red',
    running: 'amber'
};

const WorkflowExecutionHistory = ({ workflowId, onClose }) => {
    const [executions, setExecutions] = useState([]);
    const [selectedExecution, setSelectedExecution] = useState(null);
    const [logs, setLogs] = useState([]);
    const [isLoading, setIsLoading] = useState(true);
    const [isLogsLoading, setIsLogsLoading] = useState(false);

    const fetchExecutions = useCallback(async () => {
        setIsLoading(true);
        try {
            const response = await api.getWorkflowExecutions(workflowId);
            setExecutions(response.executions || []);
        } catch (error) {
            console.error('Failed to fetch executions:', error);
        } finally {
            setIsLoading(false);
        }
    }, [workflowId]);

    const fetchLogs = useCallback(async (executionId) => {
        setIsLogsLoading(true);
        try {
            const response = await api.getWorkflowExecutionLogs(executionId);
            setLogs(response.logs || []);
        } catch (error) {
            console.error('Failed to fetch logs:', error);
        } finally {
            setIsLogsLoading(false);
        }
    }, []);

    useEffect(() => {
        fetchExecutions();
    }, [fetchExecutions]);

    useEffect(() => {
        if (selectedExecution) {
            fetchLogs(selectedExecution.id);
        }
    }, [selectedExecution, fetchLogs]);

    return (
        <Modal open={true} onClose={onClose} title="Execution History" size="lg">
                <div className="wf-history">
                    {/* Executions List */}
                    <div className="wf-history__list">
                        {isLoading ? (
                            <div className="wf-history__loading"><Loader2 size={18} className="animate-spin" /></div>
                        ) : executions.length === 0 ? (
                            <div className="wf-history__empty">No executions found</div>
                        ) : (
                            executions.map((exec) => (
                                <div
                                    key={exec.id}
                                    className={`wf-history__item ${selectedExecution?.id === exec.id ? 'is-on' : ''}`}
                                    onClick={() => setSelectedExecution(exec)}
                                >
                                    <div className="wf-history__item-top">
                                        <span className="wf-history__id">#{exec.id}</span>
                                        <Pill kind={statusKind[exec.status] || 'gray'}>{exec.status}</Pill>
                                    </div>
                                    <div className="wf-history__trigger">{exec.trigger_type} trigger</div>
                                    <div className="wf-history__time">
                                        <Clock size={10} />
                                        {new Date(exec.started_at).toLocaleString()}
                                    </div>
                                </div>
                            ))
                        )}
                    </div>

                    {/* Execution Details & Logs */}
                    <div className="wf-history__detail">
                        {selectedExecution ? (
                            <>
                                <div className="wf-history__head">
                                    <div>
                                        <h3 className="wf-history__title">
                                            Execution Details #{selectedExecution.id}
                                            <Pill kind={statusKind[selectedExecution.status] || 'gray'}>
                                                {selectedExecution.status}
                                            </Pill>
                                        </h3>
                                        <div className="wf-history__dur">
                                            Duration: {selectedExecution.duration ? `${selectedExecution.duration.toFixed(2)}s` : 'N/A'}
                                        </div>
                                    </div>
                                </div>

                                <div className="wf-history__logs">
                                    <div className="wf-history__logs-label">
                                        <FileText size={13} />
                                        <span>Execution Logs</span>
                                    </div>

                                    {isLogsLoading ? (
                                        <div className="wf-history__loading"><Loader2 size={18} className="animate-spin" /></div>
                                    ) : logs.length === 0 ? (
                                        <div className="wf-history__nolog">No logs available for this execution.</div>
                                    ) : (
                                        <div className="wf-history__lines">
                                            {logs.map((log) => (
                                                <div key={log.id} className="wf-log">
                                                    <span className="wf-log__ts">[{new Date(log.timestamp).toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' })}]</span>
                                                    <span className={`wf-log__lvl wf-log__lvl--${(log.level || 'info').toLowerCase()}`}>
                                                        {log.level}
                                                    </span>
                                                    {log.node_id && (
                                                        <span className="wf-log__node">[{log.node_id}]</span>
                                                    )}
                                                    <span className="wf-log__msg">{log.message}</span>
                                                </div>
                                            ))}
                                        </div>
                                    )}
                                </div>
                            </>
                        ) : (
                            <div className="wf-history__placeholder">
                                <Activity size={44} />
                                <p>Select an execution to view details and logs</p>
                            </div>
                        )}
                    </div>
                </div>
        </Modal>
    );
};

export default WorkflowExecutionHistory;
