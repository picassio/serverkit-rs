"""
Remote Cron Service

Mirrors the local CronService surface but routes through the agent
registry so callers can target a remote server. Keeps the local-only
CronService unchanged — it still serves /api/v1/cron/* (the panel host
itself).

Action mapping (panel → agent):
    cron:status   -> Status struct {available, running, daemon, reason}
    cron:list     -> {jobs: [Entry, ...]}
    cron:add      -> Entry
    cron:remove   -> {success: true}
    cron:toggle   -> {success: true, enabled}

The agent is the source of truth for what's actually in the user's
crontab. We intentionally do NOT cache results here — every call
round-trips so the UI can't display stale state.
"""

from typing import Any, Dict, Optional

from app.services.agent_registry import agent_registry


class RemoteCronService:
    """Thin dispatcher to agent cron handlers."""

    @staticmethod
    def _send(server_id: str, action: str, params: Optional[Dict[str, Any]] = None,
              user_id: Optional[int] = None, timeout: float = 15.0) -> Dict[str, Any]:
        """Single point for cron:* dispatch. Catches the common offline
        case so the API layer can map it to a 503 without parsing
        free-form error strings."""
        return agent_registry.send_command(
            server_id=server_id,
            action=action,
            params=params or {},
            user_id=user_id,
            timeout=timeout,
        )

    @staticmethod
    def status(server_id: str, user_id: Optional[int] = None) -> Dict[str, Any]:
        return RemoteCronService._send(server_id, 'cron:status', user_id=user_id, timeout=8.0)

    @staticmethod
    def list_jobs(server_id: str, user_id: Optional[int] = None) -> Dict[str, Any]:
        return RemoteCronService._send(server_id, 'cron:list', user_id=user_id, timeout=8.0)

    @staticmethod
    def add_job(server_id: str, schedule: str, command: str,
                name: Optional[str] = None, description: Optional[str] = None,
                user_id: Optional[int] = None) -> Dict[str, Any]:
        return RemoteCronService._send(
            server_id, 'cron:add',
            params={
                'schedule': schedule,
                'command': command,
                'name': name or '',
                'description': description or '',
            },
            user_id=user_id,
        )

    @staticmethod
    def remove_job(server_id: str, job_id: str,
                   user_id: Optional[int] = None) -> Dict[str, Any]:
        return RemoteCronService._send(
            server_id, 'cron:remove',
            params={'id': job_id},
            user_id=user_id,
        )

    @staticmethod
    def toggle_job(server_id: str, job_id: str, enabled: bool,
                   user_id: Optional[int] = None) -> Dict[str, Any]:
        return RemoteCronService._send(
            server_id, 'cron:toggle',
            params={'id': job_id, 'enabled': bool(enabled)},
            user_id=user_id,
        )
