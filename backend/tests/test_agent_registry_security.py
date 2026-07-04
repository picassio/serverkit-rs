"""
Regression tests for two agent_registry defects found in the deep-dive review:

1. verify_agent_auth() must FAIL CLOSED when no decryptable API secret is on
   file. Previously the HMAC check was wrapped in `if api_secret:`, so a server
   with a missing/undecryptable secret authenticated on agent_id + api_key_prefix
   alone (both non-secret, observable values).

2. The heartbeat reaper must NOT evict an agent that reconnected between the
   stale-scan snapshot and eviction. _check_heartbeats snapshots under the lock
   then releases it; _handle_agent_timeout used to pop unconditionally, so a
   fresh reconnect (new socket_id) got clobbered and flipped offline.
"""
import time
import hmac
import hashlib
from datetime import datetime, timedelta

import pytest

from app import db as _db
from app.models.server import Server
from app.services.agent_registry import agent_registry


@pytest.fixture(autouse=True)
def _silence_side_effects(monkeypatch):
    """No-op the anomaly/nonce side-effects so these tests exercise only the
    auth decision logic, not unrelated subsystems."""
    import app.services.anomaly_detection_service as ad
    monkeypatch.setattr(ad.anomaly_detection_service, "track_auth_attempt",
                        lambda *a, **k: None, raising=False)
    monkeypatch.setattr(ad.anomaly_detection_service, "track_replay_attack",
                        lambda *a, **k: None, raising=False)


# --------------------------------------------------------------------------
# Bug #1 — mandatory signature verification
# --------------------------------------------------------------------------

def _signed(agent_id, secret, ts, nonce=None):
    msg = f"{agent_id}:{ts}:{nonce}" if nonce else f"{agent_id}:{ts}"
    return hmac.new(secret.encode(), msg.encode(), hashlib.sha256).hexdigest()


def test_auth_fails_closed_when_no_decryptable_secret(app, monkeypatch):
    """The critical fix: no secret on file => auth rejected, never bypassed."""
    monkeypatch.setattr(Server, "get_api_secret", lambda self: None)
    monkeypatch.setattr(Server, "get_pending_api_secret", lambda self: None)

    s = Server(name="t", agent_id="agent-nosecret", api_key_prefix="sk_test12345")
    _db.session.add(s)
    _db.session.commit()

    ts = int(time.time() * 1000)
    # Attacker knows the (non-secret) agent_id + prefix and sends any signature.
    result = agent_registry.verify_agent_auth(
        agent_id="agent-nosecret",
        api_key_prefix="sk_test12345",
        signature="deadbeef" * 8,
        timestamp=ts,
        nonce=None,
        ip_address="203.0.113.7",
    )
    assert result is None


def test_auth_succeeds_with_valid_signature(app, monkeypatch):
    secret = "shared-secret-value-123"
    monkeypatch.setattr(Server, "get_api_secret", lambda self: secret)

    s = Server(name="t", agent_id="agent-ok", api_key_prefix="sk_okokokok12")
    _db.session.add(s)
    _db.session.commit()

    ts = int(time.time() * 1000)
    sig = _signed("agent-ok", secret, ts)
    result = agent_registry.verify_agent_auth(
        agent_id="agent-ok",
        api_key_prefix="sk_okokokok12",
        signature=sig,
        timestamp=ts,
        nonce=None,
        ip_address="203.0.113.7",
    )
    assert result is not None
    assert result.agent_id == "agent-ok"


def test_auth_fails_with_wrong_signature(app, monkeypatch):
    monkeypatch.setattr(Server, "get_api_secret", lambda self: "the-real-secret")

    s = Server(name="t", agent_id="agent-wrong", api_key_prefix="sk_wrongwrong")
    _db.session.add(s)
    _db.session.commit()

    ts = int(time.time() * 1000)
    bad_sig = _signed("agent-wrong", "a-different-secret", ts)
    result = agent_registry.verify_agent_auth(
        agent_id="agent-wrong",
        api_key_prefix="sk_wrongwrong",
        signature=bad_sig,
        timestamp=ts,
        nonce=None,
        ip_address="203.0.113.7",
    )
    assert result is None


# --------------------------------------------------------------------------
# Bug #2 — reaper must not clobber a reconnected agent
# --------------------------------------------------------------------------

@pytest.fixture
def clean_registry():
    """Isolate the in-memory singleton registry per test."""
    with agent_registry._lock:
        agent_registry._agents.clear()
        agent_registry._socket_to_server.clear()
    yield
    with agent_registry._lock:
        agent_registry._agents.clear()
        agent_registry._socket_to_server.clear()


def _make_stale(server_id):
    with agent_registry._lock:
        agent_registry._agents[server_id].last_heartbeat = (
            datetime.utcnow() - timedelta(seconds=120)
        )


def test_reaper_does_not_evict_reconnected_agent(app, clean_registry):
    agent_registry._app = app

    s = Server(name="t", agent_id="agent-recon")
    _db.session.add(s)
    _db.session.commit()
    sid = s.id

    # Original connection, then it goes stale.
    agent_registry.register_agent(sid, "socket-OLD", "203.0.113.7", "1.0.0")
    _make_stale(sid)

    # Agent reconnects on a NEW socket (fresh heartbeat, server online again).
    agent_registry.register_agent(sid, "socket-NEW", "203.0.113.7", "1.0.0")

    # Reaper fires for the OLD snapshot it took before the reconnect.
    agent_registry._handle_agent_timeout(sid, "socket-OLD")

    # The fresh connection must survive and the server must stay online.
    assert agent_registry.is_agent_connected(sid) is True
    assert agent_registry.get_agent(sid).socket_id == "socket-NEW"
    assert Server.query.get(sid).status != "offline"


def test_reaper_evicts_genuinely_stale_agent(app, clean_registry):
    agent_registry._app = app

    s = Server(name="t", agent_id="agent-stale")
    _db.session.add(s)
    _db.session.commit()
    sid = s.id

    agent_registry.register_agent(sid, "socket-1", "203.0.113.7", "1.0.0")
    _make_stale(sid)

    # Same socket, genuinely stale -> evicted and marked offline.
    agent_registry._handle_agent_timeout(sid, "socket-1")

    assert agent_registry.is_agent_connected(sid) is False
    assert Server.query.get(sid).status == "offline"


# --------------------------------------------------------------------------
# Bug #4 — reconnect must fail in-flight commands and drop the stale socket
# --------------------------------------------------------------------------

def test_register_agent_fails_old_pending_commands_on_reconnect(app, clean_registry):
    from app.services.agent_registry import PendingCommand

    s = Server(name="t", agent_id="agent-reconpending")
    _db.session.add(s)
    _db.session.commit()
    sid = s.id

    agent_registry.register_agent(sid, "socket-OLD", "203.0.113.7", "1.0.0")
    old = agent_registry.get_agent(sid)
    pending = PendingCommand(command_id="cmd1", action="noop", params={})
    with agent_registry._lock:
        old.pending_commands["cmd1"] = pending

    # Agent reconnects on a new socket while a command is in flight.
    agent_registry.register_agent(sid, "socket-NEW", "203.0.113.7", "1.0.0")

    # The caller blocked in send_command() gets an immediate failure rather
    # than waiting out the full timeout.
    res = pending.result_queue.get(timeout=2)
    assert res["success"] is False
    assert res["code"] == "AGENT_RECONNECTED"
    assert agent_registry.get_agent(sid).socket_id == "socket-NEW"


def test_register_agent_disconnects_old_ws_socket(app, clean_registry):
    class _FakeServer:
        def __init__(self):
            self.calls = []

        def disconnect(self, sid, namespace=None):
            self.calls.append((sid, namespace))

    class _FakeSocketIO:
        def __init__(self):
            self.server = _FakeServer()

        def emit(self, *a, **k):
            pass

    fake = _FakeSocketIO()
    prev = agent_registry._socketio
    agent_registry._socketio = fake
    try:
        s = Server(name="t", agent_id="agent-recondisc")
        _db.session.add(s)
        _db.session.commit()
        sid = s.id

        agent_registry.register_agent(sid, "socket-OLD", "1.2.3.4", "1.0.0")
        agent_registry.register_agent(sid, "socket-NEW", "1.2.3.4", "1.0.0")

        assert ("socket-OLD", "/agent") in fake.server.calls
    finally:
        agent_registry._socketio = prev


# --------------------------------------------------------------------------
# Gateway hardening: the nonce must be consumed only AFTER the HMAC signature
# is verified, so a forged request can't burn a legitimate agent's nonce.
# --------------------------------------------------------------------------

def test_failed_signature_does_not_consume_nonce(app, monkeypatch):
    secret = "shared-secret-value-xyz"
    monkeypatch.setattr(Server, "get_api_secret", lambda self: secret)

    # Spy on the replay store: record every (server_id, nonce) it is asked to
    # consume. If the ordering is wrong, a bad-signature attempt consumes the
    # nonce here before the signature is ever checked.
    consumed = []
    import app.services.nonce_service as ns

    def _spy_check_and_record(server_id, nonce):
        consumed.append((server_id, nonce))
        return True

    monkeypatch.setattr(ns.nonce_service, "check_and_record",
                        _spy_check_and_record, raising=False)

    s = Server(name="t", agent_id="agent-nonce", api_key_prefix="sk_nonce12345")
    _db.session.add(s)
    _db.session.commit()

    ts = int(time.time() * 1000)
    nonce = "nonce-abc-123"

    # Forged attempt: correct agent_id/prefix/nonce, WRONG signature.
    bad = agent_registry.verify_agent_auth(
        agent_id="agent-nonce",
        api_key_prefix="sk_nonce12345",
        signature="bad" * 8,
        timestamp=ts,
        nonce=nonce,
        ip_address="203.0.113.9",
    )
    assert bad is None
    assert consumed == [], "nonce was consumed before the signature was verified"

    # The real agent then presents the same nonce with a valid signature and
    # must still succeed (the forged attempt didn't burn it).
    good_sig = _signed("agent-nonce", secret, ts, nonce)
    ok = agent_registry.verify_agent_auth(
        agent_id="agent-nonce",
        api_key_prefix="sk_nonce12345",
        signature=good_sig,
        timestamp=ts,
        nonce=nonce,
        ip_address="203.0.113.9",
    )
    assert ok is not None
    assert consumed == [(s.id, nonce)], "valid request should consume the nonce exactly once"


# --------------------------------------------------------------------------
# register_agent must fail closed on a DB error: return None and roll back the
# in-memory registration so the gateway can't report a connected agent with no
# backing session row.
# --------------------------------------------------------------------------

def test_register_agent_returns_none_and_rolls_back_on_db_failure(app, clean_registry, monkeypatch):
    import app.services.agent_registry as reg

    s = Server(name="t", agent_id="agent-dbfail")
    _db.session.add(s)
    _db.session.commit()
    sid = s.id

    # Blow up AgentSession construction to simulate a DB-layer failure that
    # happens AFTER the in-memory registration has been installed.
    class _Boom:
        def __init__(self, *a, **k):
            raise RuntimeError("simulated db failure")

    monkeypatch.setattr(reg, "AgentSession", _Boom)

    token = agent_registry.register_agent(sid, "socket-dbfail", "1.2.3.4", "1.0.0")

    assert token is None
    assert agent_registry.is_agent_connected(sid) is False
    assert agent_registry.get_agent(sid) is None
