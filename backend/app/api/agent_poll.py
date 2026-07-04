"""
Agent polling fallback transport.

Provides REST endpoints that mirror the heartbeat / command / result
exchange normally carried over Socket.IO, for agents behind tunnels that
mangle WebSocket frames (Cloudflare quick tunnels, free-tier ngrok, some
restrictive corporate proxies). Streaming features — live logs,
real-time metrics fan-out, terminal sessions — are intentionally NOT
supported here; they degrade to "view recent only" via the regular API.

Auth model:
  - POST /connect with the same HMAC payload the WS namespace uses;
    receive a session_token.
  - All subsequent calls send "X-Session-Token: <token>". The token
    looks up the ConnectedAgent in the registry (transport='poll'),
    which is what authenticates the request — no per-call HMAC.
"""

import logging
from datetime import datetime
from flask import Blueprint, jsonify, request

from app.services.agent_registry import agent_registry
from app.agent_gateway import _check_auth_rate_limit
from app.utils.ip_utils import is_ip_allowed
from app.services.anomaly_detection_service import anomaly_detection_service


logger = logging.getLogger(__name__)

agent_poll_bp = Blueprint('agent_poll', __name__)


def _client_ip():
    return request.remote_addr or 'unknown'


@agent_poll_bp.route('/connect', methods=['POST'])
def connect():
    """Authenticate an agent and return a session_token bound to a
    polling-mode ConnectedAgent. Mirrors the on_auth handler in
    AgentNamespace, minus the socket-room bookkeeping."""
    data = request.get_json(silent=True) or {}
    agent_id = data.get('agent_id')
    api_key_prefix = data.get('api_key_prefix')
    signature = data.get('signature')
    timestamp = data.get('timestamp', 0)
    nonce = data.get('nonce')

    if not all([agent_id, api_key_prefix, signature]):
        return jsonify({'success': False, 'error': 'Missing required fields'}), 400

    ip = _client_ip()

    # Apply the SAME per-IP auth throttle that guards the WebSocket auth path
    # (agent_gateway.on_auth). /connect is the REST-transport equivalent of
    # on_auth, so without this it was an unthrottled bypass an attacker could
    # use for credential-stuffing while the WS path stayed rate-limited.
    if not _check_auth_rate_limit(ip):
        logger.warning("Auth rate limit exceeded for IP: %s (poll/connect)", ip)
        return jsonify({'success': False, 'error': 'Rate limit exceeded. Try again later.'}), 429

    server = agent_registry.verify_agent_auth(
        agent_id, api_key_prefix, signature, timestamp,
        nonce=nonce, ip_address=ip,
    )
    if not server:
        return jsonify({'success': False, 'error': 'Authentication failed'}), 401

    if server.allowed_ips and len(server.allowed_ips) > 0:
        if not is_ip_allowed(ip, server.allowed_ips):
            anomaly_detection_service.track_ip_blocked(server.id, ip, server.allowed_ips)
            return jsonify({'success': False, 'error': 'IP not allowed'}), 403

    anomaly_detection_service.check_new_ip(server.id, ip)

    agent_version = (request.headers.get('User-Agent') or '').replace('ServerKit-Agent/', '')
    # Synthesize a unique socket_id so the registry's existing storage and
    # heartbeat machinery work unchanged. The "poll-" prefix is what tells
    # WS-only code paths to skip these agents (no socket_id is reachable
    # via socketio.emit).
    import secrets
    socket_id = f'poll-{secrets.token_urlsafe(12)}'

    session_token = agent_registry.register_agent(
        server_id=server.id,
        socket_id=socket_id,
        ip_address=ip,
        agent_version=agent_version,
        transport='poll',
    )

    # None means the DB write failed and register_agent rolled back the
    # in-memory state — surface a 5xx rather than a session the panel can't back.
    if not session_token:
        return jsonify({'success': False, 'error': 'Registration failed'}), 500

    return jsonify({
        'success': True,
        'session_token': session_token,
        'server_id': server.id,
        # Hint for the agent on how often to poll. Long-polls for up to
        # this many seconds before returning empty so loops are gentle.
        'poll_interval_s': 25,
    })


@agent_poll_bp.route('/poll', methods=['POST'])
def poll():
    """Long-polling endpoint. The agent posts a heartbeat (with metrics);
    we record it, then block up to ~25s waiting for queued commands. On
    return, the agent dispatches whatever commands came back and POSTs
    each result via /result.

    Body: {"metrics": {...}, "system_info": {...} (optional, sent once)}
    """
    token = request.headers.get('X-Session-Token')
    agent = agent_registry.get_agent_by_token(token)
    if not agent:
        return jsonify({'error': 'invalid session'}), 401

    body = request.get_json(silent=True) or {}
    metrics = body.get('metrics') or {}
    sysinfo = body.get('system_info')
    caps = body.get('capabilities')

    # Diagnostic: log whenever the agent ships state. Helps debug the
    # "panel shows N/A" failure mode where the agent is connected but
    # the periodic resend isn't actually reaching us.
    import logging as _logging
    _log = _logging.getLogger(__name__)
    _log.info(
        "agent /poll from %s: metrics=%s sysinfo_keys=%s caps_keys=%s",
        agent.server_id,
        bool(metrics),
        list(sysinfo.keys()) if isinstance(sysinfo, dict) else None,
        list(caps.keys()) if isinstance(caps, dict) else None,
    )

    agent_registry.update_heartbeat(agent.server_id, metrics)
    if sysinfo:
        agent_registry.update_system_info(agent.server_id, sysinfo)
    if caps:
        agent_registry.update_capabilities(agent.server_id, caps)

    # Long-poll up to 25s for any queued command. Below the typical
    # tunnel idle-timeout (Cloudflare ~100s, ngrok ~60s) so we never
    # discover a dead idle connection mid-wait.
    commands = agent_registry.drain_outbound(agent, max_wait_s=25.0)
    return jsonify({
        'commands': commands,
        # Echo the heartbeat ack so the agent can detect a still-live
        # session even when no commands arrive.
        'ack': True,
    })


@agent_poll_bp.route('/result', methods=['POST'])
def result():
    """Agent posts the outcome of a command it received from /poll.
    Wakes the synchronous send_command waiter on the panel side."""
    token = request.headers.get('X-Session-Token')
    body = request.get_json(silent=True) or {}
    if not agent_registry.deliver_result_by_token(token, body):
        return jsonify({'error': 'unknown command or session'}), 404
    return jsonify({'ok': True})


@agent_poll_bp.route('/disconnect', methods=['POST'])
def disconnect():
    """Clean shutdown when the agent is going offline or switching
    transports. Idempotent — missing token is fine."""
    token = request.headers.get('X-Session-Token')
    agent_registry.unregister_by_token(token, reason='client_disconnect')
    return jsonify({'ok': True})
