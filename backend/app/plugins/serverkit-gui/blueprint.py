"""ServerKit Agent GUI plugin — panel-side blueprint.

Acts as a thin proxy between the frontend and the agent's gui:* actions.
No frame data is stored; everything is forwarded through agent_registry.send_command.
"""
from flask import Blueprint, jsonify, request
from flask_jwt_extended import jwt_required, get_jwt_identity

from app.services.agent_registry import agent_registry
from app.models.server import Server
from app.models.user import User

gui_bp = Blueprint("server_gui", __name__)

DEFAULT_FRAME_TIMEOUT = 8.0
MAX_FRAME_TIMEOUT = 15.0


def _current_user():
    return User.query.get(get_jwt_identity())


def _server_or_404(server_id: str):
    server = Server.query.get(server_id)
    if not server:
        return None, (jsonify({"error": "Server not found"}), 404)
    return server, None


@gui_bp.route("/<server_id>/capabilities", methods=["GET"])
@jwt_required()
def capabilities(server_id):
    """Ask the agent what it can capture (display server, resolution, fps cap)."""
    user = _current_user()
    server, err = _server_or_404(server_id)
    if err:
        return err

    if server.status != "online":
        return jsonify({
            "capability": "none",
            "reason": "agent_offline",
            "synthetic_fallback": True,
        })

    result = agent_registry.send_command(
        server_id=server_id,
        action="gui:capabilities",
        params={},
        timeout=5.0,
        user_id=user.id if user else None,
    )

    if not result.get("success"):
        # Agent doesn't implement gui:capabilities — that's fine, fall back.
        return jsonify({
            "capability": "none",
            "reason": result.get("error", "unsupported"),
            "synthetic_fallback": True,
        })

    data = result.get("data") or {}
    data.setdefault("synthetic_fallback", data.get("capability") in (None, "none"))
    return jsonify(data)


@gui_bp.route("/<server_id>/frame", methods=["GET"])
@jwt_required()
def frame(server_id):
    """Capture and return a single frame.

    Query params:
      scale   float 0.1..1.0   server-side downscale before encoding
      quality int   10..95     JPEG quality (PNG ignores this)
      format  png|jpeg         encoding hint
    """
    user = _current_user()
    server, err = _server_or_404(server_id)
    if err:
        return err

    if server.status != "online":
        return jsonify({"error": "agent offline", "code": "AGENT_OFFLINE"}), 503

    try:
        scale = float(request.args.get("scale", "0.75"))
        quality = int(request.args.get("quality", "70"))
    except ValueError:
        return jsonify({"error": "scale/quality must be numeric"}), 400

    scale = max(0.1, min(scale, 1.0))
    quality = max(10, min(quality, 95))
    fmt = request.args.get("format", "jpeg").lower()
    if fmt not in ("jpeg", "png"):
        fmt = "jpeg"

    result = agent_registry.send_command(
        server_id=server_id,
        action="gui:screenshot",
        params={"scale": scale, "quality": quality, "format": fmt},
        timeout=DEFAULT_FRAME_TIMEOUT,
        user_id=user.id if user else None,
    )

    if not result.get("success"):
        return jsonify({
            "error": result.get("error", "capture failed"),
            "code": result.get("code", "CAPTURE_FAILED"),
        }), 502

    data = result.get("data") or {}
    # Expected agent shape:
    #   { "image_base64": "...", "format": "jpeg", "width": 1920, "height": 1080,
    #     "captured_at": "2026-05-01T12:34:56Z" }
    if "image_base64" not in data:
        return jsonify({"error": "agent returned no frame"}), 502

    return jsonify(data)


@gui_bp.route("/<server_id>/synthetic", methods=["GET"])
@jwt_required()
def synthetic(server_id):
    """Return data the frontend uses to render the headless 'fake desktop'.

    No new agent action — we reuse data the agent already exposes via
    existing actions. Cheap and always available.
    """
    user = _current_user()
    server, err = _server_or_404(server_id)
    if err:
        return err

    if server.status != "online":
        return jsonify({
            "windows": [],
            "taskbar": [],
            "drives": [],
            "offline": True,
        })

    sysinfo = agent_registry.send_command(
        server_id=server_id,
        action="system:info",
        params={},
        timeout=5.0,
        user_id=user.id if user else None,
    )
    procs = agent_registry.send_command(
        server_id=server_id,
        action="system:processes",
        params={"limit": 12},
        timeout=5.0,
        user_id=user.id if user else None,
    )

    info = (sysinfo.get("data") or {}) if sysinfo.get("success") else {}
    plist = (procs.get("data") or []) if procs.get("success") else []

    windows = [
        {
            "id": "system",
            "title": f"System — {info.get('hostname', server.name)}",
            "icon": "monitor",
            "body": {
                "OS": f"{info.get('os', 'Unknown')} {info.get('os_version', '')}".strip(),
                "Arch": info.get("architecture", "?"),
                "CPU": info.get("cpu_model", "?"),
                "Cores": info.get("cpu_cores", "?"),
            },
        },
        {
            "id": "processes",
            "title": "Top processes",
            "icon": "activity",
            "body": [
                {"name": p.get("name"), "cpu": p.get("cpu_percent"), "mem": p.get("memory_percent")}
                for p in plist[:8]
            ],
        },
    ]

    taskbar = [
        {"id": p.get("pid"), "name": p.get("name", "?")}
        for p in plist[:6]
    ]

    drives = [
        {"path": d.get("mountpoint"), "used": d.get("used_percent")}
        for d in (info.get("disks") or [])
    ]

    return jsonify({
        "windows": windows,
        "taskbar": taskbar,
        "drives": drives,
        "hostname": info.get("hostname") or server.name,
        "offline": False,
    })
