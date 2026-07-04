"""
Advanced Workflow & Automation Engine

Executes event-driven workflows with DAG-based execution, logic branching,
variable interpolation, timeouts, retries, and script sandboxing.
"""

import json
import logging
import os
import re
import signal
import subprocess
import threading
import traceback
from collections import deque
from datetime import datetime
from typing import Dict, List, Any, Optional, Set, Tuple

from app import db
from app.models import Workflow, WorkflowExecution, WorkflowLog, User
from app.services.workflow_service import WorkflowService
from app.services.docker_service import DockerService
from app.services.database_service import DatabaseService
from app.services.notification_service import NotificationService

logger = logging.getLogger(__name__)

# Defaults for node execution
DEFAULT_TIMEOUT = 300  # 5 minutes
MAX_TIMEOUT = 3600  # 1 hour
DEFAULT_RETRY_COUNT = 0
MAX_RETRY_COUNT = 5
DEFAULT_RETRY_DELAY = 5  # seconds
MAX_OUTPUT_SIZE = 1024 * 512  # 512 KB

# Unified job kinds for asynchronous workflow execution + event dispatch
# (see WorkflowEngine.enqueue_execution / register_jobs).
WORKFLOW_JOB_KIND = 'workflow.execute'
WORKFLOW_DISPATCH_JOB_KIND = 'workflow.dispatch'


def _parse_version(s: str) -> tuple:
    """Turn '3.11.4' / 'v20.10.0' / '8.2' into a comparable tuple of ints.
    Trailing non-numeric parts are dropped silently."""
    if not s:
        return ()
    s = s.lstrip('vV').strip()
    parts = []
    for part in s.split('.'):
        digits = ''.join(c for c in part if c.isdigit())
        if not digits:
            break
        parts.append(int(digits))
    return tuple(parts)


def _runtime_satisfies(actual: str, spec: str) -> bool:
    """Compare actual version against a spec like '>=3.11', '>=18',
    '==8.2', or just '3.11' (treated as >=). Naive but covers the
    realistic gate cases without an extra dep."""
    spec = spec.strip()
    op = '>='
    for prefix in ('>=', '<=', '==', '>', '<'):
        if spec.startswith(prefix):
            op = prefix
            spec = spec[len(prefix):].strip()
            break
    a = _parse_version(actual)
    b = _parse_version(spec)
    if not a or not b:
        return False
    if op == '>=':
        return a >= b
    if op == '<=':
        return a <= b
    if op == '==':
        return a == b
    if op == '>':
        return a > b
    if op == '<':
        return a < b
    return False


class CycleDetectedError(Exception):
    """Raised when a cycle is detected in the workflow graph."""
    pass


class NodeTimeoutError(Exception):
    """Raised when a node exceeds its timeout."""
    pass


class WorkflowEngine:
    """Engine for executing advanced workflows with DAG support."""

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    @staticmethod
    def validate_graph(nodes: List[Dict], edges: List[Dict]) -> Optional[str]:
        """
        Validate a workflow graph for cycles.

        Returns None if valid, or an error message string if a cycle is found.
        """
        adj: Dict[str, List[str]] = {}
        node_ids = {n['id'] for n in nodes}

        for edge in edges:
            src = edge['source']
            if src not in adj:
                adj[src] = []
            adj[src].append(edge['target'])

        # Kahn's algorithm for cycle detection
        in_degree = {nid: 0 for nid in node_ids}
        for src, targets in adj.items():
            for t in targets:
                if t in in_degree:
                    in_degree[t] += 1

        queue = deque(nid for nid, deg in in_degree.items() if deg == 0)
        visited_count = 0

        while queue:
            nid = queue.popleft()
            visited_count += 1
            for neighbor in adj.get(nid, []):
                if neighbor in in_degree:
                    in_degree[neighbor] -= 1
                    if in_degree[neighbor] == 0:
                        queue.append(neighbor)

        if visited_count < len(node_ids):
            # Find nodes involved in the cycle for a helpful message
            cycle_nodes = [nid for nid, deg in in_degree.items() if deg > 0]
            node_labels = {}
            for n in nodes:
                node_labels[n['id']] = n.get('data', {}).get('label', n['id'])
            cycle_labels = [node_labels.get(nid, nid) for nid in cycle_nodes[:5]]
            return f"Cycle detected involving: {', '.join(cycle_labels)}"

        return None

    @staticmethod
    def execute_workflow(workflow_id: int, trigger_type: str = 'manual',
                         context: Dict[str, Any] = None) -> int:
        """Create and SYNCHRONOUSLY run a workflow. Returns the execution id.

        Kept for callers that want to block on the result (and tests). Async
        callers (API/webhook/cron/event) use enqueue_execution instead.
        """
        execution = WorkflowEngine._create_execution(workflow_id, trigger_type, context)
        WorkflowEngine.run_execution(execution.id)
        return execution.id

    @staticmethod
    def enqueue_execution(workflow_id: int, trigger_type: str = 'manual',
                          context: Dict[str, Any] = None) -> int:
        """Create the execution row and run it asynchronously via the unified job
        system (kind ``workflow.execute``). Returns the execution id immediately,
        so triggers don't block on the DAG. Graph validation still happens here,
        so an invalid workflow raises synchronously (callers can 400)."""
        execution = WorkflowEngine._create_execution(workflow_id, trigger_type, context)
        from app.jobs.service import JobService
        JobService.enqueue(
            WORKFLOW_JOB_KIND,
            payload={'execution_id': execution.id},
            max_attempts=1,  # workflows aren't idempotent (deploy/script/notify)
            owner_type='workflow',
            owner_id=workflow_id,
        )
        return execution.id

    @staticmethod
    def _create_execution(workflow_id: int, trigger_type: str = 'manual',
                          context: Dict[str, Any] = None) -> WorkflowExecution:
        """Validate the workflow graph and persist a new (running) execution row."""
        workflow = Workflow.query.get(workflow_id)
        if not workflow:
            raise ValueError(f"Workflow {workflow_id} not found")

        nodes = json.loads(workflow.nodes) if workflow.nodes else []
        edges = json.loads(workflow.edges) if workflow.edges else []

        # Validate graph before creating the execution
        cycle_err = WorkflowEngine.validate_graph(nodes, edges)
        if cycle_err:
            raise CycleDetectedError(cycle_err)

        execution = WorkflowExecution(
            workflow_id=workflow_id,
            trigger_type=trigger_type,
            status='running',
            context=json.dumps(context or {}),
            started_at=datetime.utcnow()
        )
        db.session.add(execution)
        db.session.commit()

        workflow.last_run_at = execution.started_at
        db.session.commit()
        return execution

    @staticmethod
    def run_execution(execution_id: int) -> int:
        """Run an already-created execution to completion. The synchronous core
        shared by execute_workflow and the workflow.execute job handler."""
        execution = WorkflowExecution.query.get(execution_id)
        if not execution:
            raise ValueError(f"WorkflowExecution {execution_id} not found")
        workflow = execution.workflow
        nodes = json.loads(workflow.nodes) if workflow.nodes else []
        edges = json.loads(workflow.edges) if workflow.edges else []

        try:
            WorkflowEngine._run_execution(execution.id, nodes, edges)
        except Exception as e:
            WorkflowEngine._log(execution.id, f"Engine Error: {str(e)}", level='ERROR')
            execution.status = 'failed'
            execution.completed_at = datetime.utcnow()
            workflow.last_status = 'failed'
            db.session.commit()

        return execution.id

    @staticmethod
    def _run_workflow_job(job):
        """Unified-job handler for ``workflow.execute``. Runs the queued execution;
        a failed run is raised so the unified job is marked failed too (the
        WorkflowExecution row carries the detailed per-node status/logs)."""
        execution_id = (job.get_payload() or {}).get('execution_id')
        if not execution_id:
            raise ValueError('workflow.execute job missing execution_id')
        WorkflowEngine.run_execution(execution_id)
        execution = WorkflowExecution.query.get(execution_id)
        if execution and execution.status == 'failed':
            raise RuntimeError(f'Workflow execution {execution_id} failed')
        return {'execution_id': execution_id,
                'status': execution.status if execution else None}

    @staticmethod
    def register_jobs():
        """Register the workflow job handlers with the unified job registry.
        Called once at app startup (see app/__init__.py)."""
        from app.jobs import registry
        registry.register(WORKFLOW_JOB_KIND, WorkflowEngine._run_workflow_job, replace=True)
        registry.register(WORKFLOW_DISPATCH_JOB_KIND, WorkflowEventBus.dispatch_event, replace=True)

    # ------------------------------------------------------------------
    # DAG Execution
    # ------------------------------------------------------------------

    @staticmethod
    def _run_execution(execution_id: int, nodes: List[Dict], edges: List[Dict]):
        """Run the workflow using topological DAG execution with branch support."""
        execution = WorkflowExecution.query.get(execution_id)
        workflow = execution.workflow

        WorkflowEngine._log(execution_id, f"Starting workflow: {workflow.name}")

        if not nodes:
            WorkflowEngine._log(execution_id, "Workflow has no nodes", level='WARNING')
            execution.status = 'success'
            execution.completed_at = datetime.utcnow()
            db.session.commit()
            return

        # Build graph structures
        node_map = {n['id']: n for n in nodes}
        adj: Dict[str, List[Tuple[str, str]]] = {}  # source -> [(target, sourceHandle)]
        in_degree: Dict[str, int] = {n['id']: 0 for n in nodes}

        for edge in edges:
            src = edge['source']
            tgt = edge['target']
            src_handle = edge.get('sourceHandle', 'output')
            if src not in adj:
                adj[src] = []
            adj[src].append((tgt, src_handle))
            if tgt in in_degree:
                in_degree[tgt] += 1

        # Start with root nodes (no incoming edges)
        ready = deque(nid for nid, deg in in_degree.items() if deg == 0)
        if not ready:
            ready.append(nodes[0]['id'])

        context = json.loads(execution.context) if execution.context else {}
        results: Dict[str, Dict] = {}
        processed: Set[str] = set()
        # Track which branches are active (for logic_if gating)
        # Maps node_id -> set of sourceHandles that were activated
        active_branches: Dict[str, Set[str]] = {}
        failed = False

        while ready and not failed:
            node_id = ready.popleft()

            if node_id in processed:
                continue

            node = node_map.get(node_id)
            if not node:
                continue

            # Check if this node is gated by a logic_if branch
            if not WorkflowEngine._is_node_reachable(node_id, edges, active_branches, processed):
                processed.add(node_id)
                # Still decrement successors so they can become ready
                for tgt, _ in adj.get(node_id, []):
                    if tgt in in_degree:
                        in_degree[tgt] -= 1
                        if in_degree[tgt] == 0:
                            ready.append(tgt)
                continue

            node_label = node.get('data', {}).get('label', node_id)
            WorkflowEngine._log(execution_id, f"Executing node: {node_label} ({node['type']})", node_id=node_id)

            try:
                node_result = WorkflowEngine._execute_node_with_retry(
                    node, edges, execution, context, results
                )
                results[node_id] = node_result

                if not node_result.get('success', True):
                    WorkflowEngine._log(
                        execution_id,
                        f"Node failed: {node_result.get('error', 'unknown')}",
                        level='ERROR', node_id=node_id
                    )
                    if node_result.get('critical', True):
                        failed = True
                        break

                # For logic_if nodes, record which branch was taken
                if node['type'] == 'logic_if':
                    branch = node_result.get('branch', 'true')
                    if node_id not in active_branches:
                        active_branches[node_id] = set()
                    active_branches[node_id].add(branch)

                # Enqueue successor nodes whose in-degree reaches 0
                for tgt, src_handle in adj.get(node_id, []):
                    if tgt in in_degree:
                        in_degree[tgt] -= 1
                        if in_degree[tgt] == 0:
                            ready.append(tgt)

            except Exception as e:
                WorkflowEngine._log(execution_id, f"Node Execution Error: {str(e)}", level='ERROR', node_id=node_id)
                WorkflowEngine._log(execution_id, traceback.format_exc(), level='DEBUG', node_id=node_id)
                failed = True
                break

            processed.add(node_id)

        execution.status = 'failed' if failed else 'success'
        execution.results = json.dumps(results)
        execution.completed_at = datetime.utcnow()
        workflow.last_status = execution.status
        db.session.commit()

        WorkflowEngine._log(execution_id, f"Workflow finished with status: {execution.status}")

    @staticmethod
    def _is_node_reachable(node_id: str, edges: List[Dict],
                           active_branches: Dict[str, Set[str]],
                           processed: Set[str]) -> bool:
        """
        Check if a node should execute based on logic_if branching.

        A node is unreachable if ALL its incoming edges from logic_if nodes
        come through inactive branches.
        """
        incoming_from_logic = []

        for edge in edges:
            if edge['target'] != node_id:
                continue
            src = edge['source']
            src_handle = edge.get('sourceHandle', 'output')

            # Only gate on logic_if nodes that have already been processed
            if src in active_branches:
                incoming_from_logic.append((src, src_handle))

        if not incoming_from_logic:
            return True  # No logic_if gating, always reachable

        # Reachable if at least one logic_if branch leading here is active
        for src, src_handle in incoming_from_logic:
            if src_handle in active_branches[src]:
                return True

        return False

    # ------------------------------------------------------------------
    # Node Execution with Retry
    # ------------------------------------------------------------------

    @staticmethod
    def _execute_node_with_retry(node: Dict, edges: List, execution: WorkflowExecution,
                                  context: Dict, results: Dict) -> Dict:
        """Execute a node with retry support."""
        node_data = node.get('data', {})
        retry_count = min(int(node_data.get('retryCount', DEFAULT_RETRY_COUNT)), MAX_RETRY_COUNT)
        retry_delay = max(1, int(node_data.get('retryDelay', DEFAULT_RETRY_DELAY)))

        last_result = None
        for attempt in range(retry_count + 1):
            if attempt > 0:
                WorkflowEngine._log(
                    execution.id,
                    f"Retry {attempt}/{retry_count} after {retry_delay}s",
                    node_id=node['id']
                )
                import time
                time.sleep(retry_delay)

            last_result = WorkflowEngine._execute_node(node, edges, execution, context, results)

            if last_result.get('success', True):
                return last_result

        return last_result

    # ------------------------------------------------------------------
    # Node Execution
    # ------------------------------------------------------------------

    @staticmethod
    def _execute_node(node: Dict, edges: List, execution: WorkflowExecution,
                      context: Dict, results: Dict) -> Dict:
        """Execute a single node and return its results."""
        node_type = node.get('type')
        node_data = node.get('data', {})

        if node_type == 'trigger':
            return {'success': True, 'output': context}

        elif node_type in ('database', 'dockerApp', 'service', 'domain'):
            res = WorkflowService.deploy_node(node, edges, execution.workflow.user_id, results)
            return res

        elif node_type == 'notification':
            return WorkflowEngine._execute_notification(node, execution, context, results)

        elif node_type == 'script':
            return WorkflowEngine._execute_script(node, execution, context, results)

        elif node_type == 'logic_if':
            return WorkflowEngine._execute_logic_if(node, context, results)

        # Phase 4 — fleet workflow primitives. Each routes to an agent
        # via the registry; on failure they short-circuit the workflow
        # so a missing capability or wrong runtime doesn't silently
        # mutate the target host.
        elif node_type == 'agent_command':
            return WorkflowEngine._execute_agent_command(node, execution, context, results)
        elif node_type == 'capability_gate':
            return WorkflowEngine._execute_capability_gate(node)
        elif node_type == 'runtime_gate':
            return WorkflowEngine._execute_runtime_gate(node)

        return {'success': True, 'message': f"Node type {node_type} passed through"}

    # ------------------------------------------------------------------
    # Phase 4: Agent Command + Gate Execution
    # ------------------------------------------------------------------

    @staticmethod
    def _resolve_target(node_data: Dict) -> Optional[str]:
        """Pull a `{kind, server_id}` target from node_data and return
        the server_id for agent dispatch, or None for the local panel
        host. Raises if the target dict is malformed."""
        target = node_data.get('target') or {'kind': 'local'}
        kind = target.get('kind')
        if kind == 'local' or kind is None:
            return None
        if kind == 'agent':
            sid = target.get('server_id')
            if not sid:
                raise ValueError("agent target missing server_id")
            return sid
        # 'agent_group' lands when multi-target execution does — for
        # now reject so a user authoring a template against a
        # not-yet-implemented mode gets a clear error instead of
        # silent passthrough.
        raise ValueError(f"unsupported target kind: {kind}")

    @staticmethod
    def _execute_agent_command(node: Dict, execution: WorkflowExecution,
                               context: Dict, results: Dict) -> Dict:
        """Dispatch an action to an agent and return the result. The
        node's data carries the action verb, params, and target — see
        plan §Phase 4 for the schema."""
        from app.services.agent_registry import agent_registry

        node_data = node.get('data', {})
        action = (node_data.get('action') or '').strip()
        if not action:
            return {'success': False, 'error': 'agent_command node missing action'}

        try:
            server_id = WorkflowEngine._resolve_target(node_data)
        except ValueError as e:
            return {'success': False, 'error': str(e)}

        if server_id is None:
            # Local target on an agent_command node is unusual — the
            # equivalent panel-local action belongs in the original
            # node types. Treat as a config error rather than guessing.
            return {
                'success': False,
                'error': "agent_command requires an agent target; use the matching local node type instead",
            }

        # Interpolate params so templates can reference earlier results.
        raw_params = node_data.get('params') or {}
        params: Dict[str, Any] = {}
        for k, v in raw_params.items():
            if isinstance(v, str):
                params[k] = WorkflowEngine._interpolate(v, context, results, execution)
            else:
                params[k] = v

        timeout = float(node_data.get('timeout_s') or DEFAULT_TIMEOUT)
        timeout = min(timeout, MAX_TIMEOUT)

        WorkflowEngine._log(
            execution.id,
            f"agent {server_id}: {action} {params}",
            node_id=node.get('id'),
        )

        result = agent_registry.send_command(
            server_id=server_id,
            action=action,
            params=params,
            user_id=execution.workflow.user_id if execution.workflow else None,
            timeout=timeout,
        )

        # Translate agent_registry's envelope into the engine's
        # standard {success, output|error} shape so downstream nodes
        # can branch on it uniformly.
        if not result.get('success'):
            on_failure = (node_data.get('on_failure') or 'abort').lower()
            return {
                'success': False,
                'error': result.get('error') or 'agent command failed',
                'code': result.get('code'),
                'critical': on_failure == 'abort',
            }
        return {
            'success': True,
            'output': result.get('data'),
            'action': action,
            'server_id': server_id,
        }

    @staticmethod
    def _execute_capability_gate(node: Dict) -> Dict:
        """Pass the workflow only if the target server reports every
        listed capability. A gate failure aborts by default — gates
        exist precisely to prevent the next step from running on an
        unsupported host."""
        from app.services.agent_registry import agent_registry

        node_data = node.get('data', {})
        try:
            server_id = WorkflowEngine._resolve_target(node_data)
        except ValueError as e:
            return {'success': False, 'error': str(e), 'critical': True}

        require = node_data.get('require') or []
        if isinstance(require, str):
            require = [require]
        require = [str(r).strip() for r in require if str(r).strip()]
        if not require:
            return {'success': True, 'message': 'no capabilities required'}

        if server_id is None:
            # Panel host — every "capability" we ask about is implicitly
            # available (it's the panel itself). Tighter modelling can
            # come later; refusing here would break local-only workflows
            # that want to use the same gate node for clarity.
            return {'success': True, 'satisfied': require, 'target': 'local'}

        caps = agent_registry.get_capabilities(server_id) or {}
        missing = [r for r in require if not caps.get(r)]
        if missing:
            on_missing = (node_data.get('on_missing') or 'abort').lower()
            return {
                'success': False,
                'error': f"target missing capabilities: {', '.join(missing)}",
                'missing': missing,
                'satisfied': [r for r in require if caps.get(r)],
                'critical': on_missing == 'abort',
            }
        return {'success': True, 'satisfied': require, 'server_id': server_id}

    @staticmethod
    def _execute_runtime_gate(node: Dict) -> Dict:
        """Pass the workflow only if every required runtime is present
        at >= the requested version. Comparison is naive lexicographic
        on tuples-of-ints — good enough for "python >= 3.11" style
        gates without dragging in a semver dependency."""
        from app.services.agent_registry import agent_registry

        node_data = node.get('data', {})
        try:
            server_id = WorkflowEngine._resolve_target(node_data)
        except ValueError as e:
            return {'success': False, 'error': str(e), 'critical': True}

        require = node_data.get('require') or {}
        if not isinstance(require, dict) or not require:
            return {'success': True, 'message': 'no runtimes required'}

        if server_id is None:
            # Local panel — we don't track runtime versions for the
            # panel itself; accept and move on.
            return {'success': True, 'target': 'local'}

        # agent_registry stores the runtime map alongside capabilities.
        agent = agent_registry._agents.get(server_id) if hasattr(agent_registry, '_agents') else None
        runtimes = dict(getattr(agent, 'runtimes', {})) if agent else {}

        unmet = []
        for name, spec in require.items():
            actual = runtimes.get(name) or ''
            if not _runtime_satisfies(actual, str(spec)):
                unmet.append({'runtime': name, 'required': spec, 'actual': actual or 'not installed'})

        if unmet:
            return {
                'success': False,
                'error': 'runtime requirements not met',
                'unmet': unmet,
                'critical': True,
            }
        return {'success': True, 'satisfied': require}

    # ------------------------------------------------------------------
    # Logic If Evaluation
    # ------------------------------------------------------------------

    @staticmethod
    def _execute_logic_if(node: Dict, context: Dict, results: Dict) -> Dict:
        """
        Evaluate a logic_if condition.

        The condition is a Python expression that has access to:
        - results: dict of {node_id: node_result}
        - context: the workflow execution context
        """
        node_data = node.get('data', {})
        condition = node_data.get('condition', '').strip()

        if not condition:
            return {'success': True, 'branch': 'true'}

        # Build a safe evaluation namespace
        eval_globals = {"__builtins__": {}}
        eval_locals = {
            'results': results,
            'context': context,
            # Expose common helpers
            'len': len,
            'str': str,
            'int': int,
            'float': float,
            'bool': bool,
            'abs': abs,
            'min': min,
            'max': max,
            'any': any,
            'all': all,
            'isinstance': isinstance,
        }

        try:
            result = eval(condition, eval_globals, eval_locals)
            branch = 'true' if result else 'false'
            return {
                'success': True,
                'branch': branch,
                'condition': condition,
                'evaluated': bool(result)
            }
        except Exception as e:
            return {
                'success': False,
                'error': f"Condition evaluation failed: {str(e)}",
                'condition': condition,
                'branch': 'false',
                'critical': False  # Don't kill the whole workflow for a bad condition
            }

    # ------------------------------------------------------------------
    # Variable Interpolation
    # ------------------------------------------------------------------

    @staticmethod
    def _interpolate(text: str, context: Dict, results: Dict,
                     execution: Optional[WorkflowExecution] = None) -> str:
        """
        Replace variable placeholders in text.

        Supported syntax:
        - ${node_id.field}     — access a specific field from a node's result
        - ${node_id.output}    — shorthand for the output field
        - {{workflow_name}}    — built-in workflow variables
        - {{execution_id}}     — current execution ID
        - {{started_at}}       — execution start time
        - {{context.field}}    — access context fields
        """
        if not text or not isinstance(text, str):
            return text

        # Replace ${node_id.field} patterns
        def replace_node_var(match):
            node_id = match.group(1)
            field = match.group(2)
            node_result = results.get(node_id, {})
            if field == 'output' and 'output' not in node_result:
                # Try stdout as fallback for script nodes
                return str(node_result.get('stdout', ''))
            return str(node_result.get(field, ''))

        text = re.sub(r'\$\{([^.}]+)\.([^}]+)\}', replace_node_var, text)

        # Replace {{builtin}} patterns
        builtins = {
            'workflow_name': execution.workflow.name if execution else '',
            'execution_id': str(execution.id) if execution else '',
            'started_at': execution.started_at.isoformat() if execution and execution.started_at else '',
            'trigger_type': execution.trigger_type if execution else '',
        }

        # Add context.* variables
        for key, value in context.items():
            builtins[f'context.{key}'] = str(value)

        for key, value in builtins.items():
            text = text.replace('{{' + key + '}}', value)

        # Replace {{node_id.field}} as alternative syntax
        def replace_node_var_braces(match):
            node_id = match.group(1)
            field = match.group(2)
            node_result = results.get(node_id, {})
            return str(node_result.get(field, ''))

        text = re.sub(r'\{\{([^.}]+)\.([^}]+)\}\}', replace_node_var_braces, text)

        return text

    # ------------------------------------------------------------------
    # Notification Node
    # ------------------------------------------------------------------

    @staticmethod
    def _execute_notification(node: Dict, execution: WorkflowExecution,
                              context: Dict, results: Dict) -> Dict:
        """Execute a notification node with variable interpolation."""
        node_data = node.get('data', {})
        channel = node_data.get('channel', 'system')
        message = node_data.get('message', 'Workflow notification')

        # Interpolate variables in the message
        message = WorkflowEngine._interpolate(message, context, results, execution)

        title = f"Workflow: {execution.workflow.name}"

        # Build alert in the format NotificationService expects
        alerts = [{
            'type': 'workflow',
            'severity': 'info',
            'message': message,
            'value': '',
            'threshold': ''
        }]

        try:
            if channel == 'system' or channel == 'all':
                result = NotificationService.send_all(alerts)
            elif channel == 'discord':
                config = NotificationService.get_config().get('discord', {})
                result = NotificationService.send_discord(alerts, config)
            elif channel == 'slack':
                config = NotificationService.get_config().get('slack', {})
                result = NotificationService.send_slack(alerts, config)
            elif channel == 'email':
                config = NotificationService.get_config().get('email', {})
                result = NotificationService.send_email(alerts, config)
            elif channel == 'telegram':
                config = NotificationService.get_config().get('telegram', {})
                result = NotificationService.send_telegram(alerts, config)
            else:
                result = NotificationService.send_all(alerts)

            return {'success': result.get('success', True), 'channel': channel}
        except Exception as e:
            return {'success': False, 'error': str(e), 'critical': False}

    # ------------------------------------------------------------------
    # Script Node (Sandboxed)
    # ------------------------------------------------------------------

    @staticmethod
    def _execute_script(node: Dict, execution: WorkflowExecution,
                        context: Dict, results: Dict) -> Dict:
        """Execute a script node with timeout, output limits, and variable interpolation."""
        node_data = node.get('data', {})
        script_type = node_data.get('language', 'bash')
        content = node_data.get('content', '')
        timeout = min(int(node_data.get('timeout', DEFAULT_TIMEOUT)), MAX_TIMEOUT)

        if not content.strip():
            return {'success': True, 'stdout': '', 'stderr': '', 'returncode': 0}

        # Interpolate variables in script content
        content = WorkflowEngine._interpolate(content, context, results, execution)

        # Build environment with node results available as env vars
        env = os.environ.copy()
        env['WORKFLOW_ID'] = str(execution.workflow_id)
        env['EXECUTION_ID'] = str(execution.id)
        env['TRIGGER_TYPE'] = execution.trigger_type or 'manual'

        for nid, nresult in results.items():
            safe_id = re.sub(r'[^a-zA-Z0-9_]', '_', nid).upper()
            if isinstance(nresult, dict):
                stdout = nresult.get('stdout', nresult.get('output', ''))
                if isinstance(stdout, str):
                    env[f'NODE_{safe_id}_OUTPUT'] = stdout[:4096]
                rc = nresult.get('returncode')
                if rc is not None:
                    env[f'NODE_{safe_id}_RC'] = str(rc)

        try:
            if script_type == 'bash':
                cmd = ['bash', '-c', content]
            elif script_type == 'python':
                cmd = ['python3', '-c', content]
            else:
                return {'success': False, 'error': f"Unsupported script language: {script_type}"}

            proc = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=timeout,
                env=env,
                cwd='/tmp' if os.name != 'nt' else None
            )

            stdout = proc.stdout[:MAX_OUTPUT_SIZE] if proc.stdout else ''
            stderr = proc.stderr[:MAX_OUTPUT_SIZE] if proc.stderr else ''

            return {
                'success': proc.returncode == 0,
                'stdout': stdout,
                'stderr': stderr,
                'returncode': proc.returncode
            }

        except subprocess.TimeoutExpired:
            return {
                'success': False,
                'error': f"Script timed out after {timeout}s",
                'stdout': '',
                'stderr': '',
                'returncode': -1
            }
        except FileNotFoundError:
            fallback = 'python' if script_type == 'python' else script_type
            try:
                proc = subprocess.run(
                    [fallback, '-c', content] if script_type == 'python' else content,
                    capture_output=True, text=True, timeout=timeout,
                    env=env, shell=(script_type == 'bash'),
                    cwd='/tmp' if os.name != 'nt' else None
                )
                stdout = proc.stdout[:MAX_OUTPUT_SIZE] if proc.stdout else ''
                stderr = proc.stderr[:MAX_OUTPUT_SIZE] if proc.stderr else ''
                return {
                    'success': proc.returncode == 0,
                    'stdout': stdout, 'stderr': stderr,
                    'returncode': proc.returncode
                }
            except Exception as e:
                return {'success': False, 'error': str(e)}
        except Exception as e:
            return {'success': False, 'error': str(e)}

    # ------------------------------------------------------------------
    # Logging
    # ------------------------------------------------------------------

    @staticmethod
    def _log(execution_id: int, message: str, level: str = 'INFO', node_id: str = None):
        """Add a log entry for an execution."""
        log_entry = WorkflowLog(
            execution_id=execution_id,
            level=level,
            message=message,
            node_id=node_id
        )
        db.session.add(log_entry)
        db.session.commit()
        logger.info(f"[{level}] Workflow {execution_id}: {message}")

    # Keep backward-compatible alias
    log = _log


# ------------------------------------------------------------------
# Event Bus for workflow triggers
# ------------------------------------------------------------------

class WorkflowEventBus:
    """
    Simple in-process event bus for triggering workflows on system events.

    Events are emitted by services (monitoring, health checks, git deploy)
    and matched against workflows with trigger_type='event'.
    """

    _listeners_lock = threading.Lock()

    @staticmethod
    def emit(event_type: str, data: Dict[str, Any] = None):
        """Emit a system event that may trigger event-subscribed workflows.

        Non-blocking: enqueues a single ``workflow.dispatch`` job that fans out to
        the matching workflows on the unified job system (replacing the former
        per-event daemon thread). Best-effort — never raises into the caller.

        Args:
            event_type: One of health_check_failed, high_cpu, high_memory,
                        git_push, app_stopped, or any custom string.
            data: Event payload passed as workflow context.
        """
        try:
            from app.jobs.service import JobService
            JobService.enqueue(
                WORKFLOW_DISPATCH_JOB_KIND,
                payload={'event_type': event_type, 'data': data or {}},
                max_attempts=1,
                owner_type='workflow_event',
                owner_id=event_type,
            )
        except Exception as e:
            # No app context, or the queue is unavailable — never break the caller.
            logger.warning(f"WorkflowEventBus.emit could not enqueue '{event_type}': {e}")

    @staticmethod
    def dispatch_event(job):
        """Unified-job handler for ``workflow.dispatch`` — find workflows
        subscribed to the event and enqueue a workflow.execute job for each
        (honoring the per-workflow 60s cooldown)."""
        payload = job.get_payload() or {}
        event_type = payload.get('event_type')
        data = payload.get('data') or {}
        if not event_type:
            return {'event_type': None, 'triggered': 0}

        workflows = Workflow.query.filter_by(is_active=True, trigger_type='event').all()
        triggered = 0
        for workflow in workflows:
            try:
                config = json.loads(workflow.trigger_config) if workflow.trigger_config else {}
                if config.get('eventType', '') != event_type:
                    continue
                # Cooldown: don't re-trigger within 60 seconds.
                if workflow.last_run_at and \
                        (datetime.utcnow() - workflow.last_run_at).total_seconds() < 60:
                    continue
                logger.info(f"Event '{event_type}' triggering workflow: {workflow.name}")
                WorkflowEngine.enqueue_execution(
                    workflow_id=workflow.id,
                    trigger_type='event',
                    context={
                        'event_type': event_type,
                        'event_data': data,
                        'triggered_at': datetime.utcnow().isoformat(),
                    },
                )
                triggered += 1
            except Exception as e:
                logger.error(f"Event trigger failed for workflow {workflow.id}: {e}")
        return {'event_type': event_type, 'triggered': triggered}
