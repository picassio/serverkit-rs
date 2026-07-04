"""
Tests for Ed25519 proof-of-possession in the short-code pairing poll (bug D).

The agent enrolls a hex-encoded Ed25519 pubkey, then signs the enrollment_id
with the matching private key on each poll. The panel verifies that signature
when present, so a stolen enrollment_secret alone can't claim a device whose
private key the attacker lacks. Verification is best-effort for backward
compatibility: a poll with no signature still works (older agents), but a
present-but-invalid signature is rejected.
"""
import base64

import pytest
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

from app.services import pairing_service
from app.services.pairing_service import InvalidEnrollmentError


def _enroll():
    """Enroll a fresh keypair; return (private_key, enrollment dict)."""
    priv = Ed25519PrivateKey.generate()
    raw_pub = priv.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    pubkey_hex = raw_pub.hex()
    enr = pairing_service.enroll(pubkey_hex, "pass1234", "machine-pop", {})
    return priv, enr


def _sig(priv, enrollment_id):
    return base64.b64encode(priv.sign(enrollment_id.encode())).decode()


def test_poll_accepts_valid_signature(app):
    priv, enr = _enroll()
    eid, esec = enr['enrollment_id'], enr['enrollment_secret']

    res = pairing_service.poll(eid, esec, signature=_sig(priv, eid))
    # Not claimed yet, but the signature was accepted (no exception raised).
    assert res['status'] == 'pending'


def test_poll_rejects_invalid_signature(app):
    priv, enr = _enroll()
    eid, esec = enr['enrollment_id'], enr['enrollment_secret']
    bad_sig = base64.b64encode(b'\x00' * 64).decode()

    with pytest.raises(InvalidEnrollmentError):
        pairing_service.poll(eid, esec, signature=bad_sig)


def test_poll_rejects_signature_from_wrong_key(app):
    _priv, enr = _enroll()
    eid, esec = enr['enrollment_id'], enr['enrollment_secret']
    attacker = Ed25519PrivateKey.generate()
    wrong_sig = base64.b64encode(attacker.sign(eid.encode())).decode()

    with pytest.raises(InvalidEnrollmentError):
        pairing_service.poll(eid, esec, signature=wrong_sig)


def test_poll_without_signature_is_backward_compatible(app):
    _priv, enr = _enroll()
    eid, esec = enr['enrollment_id'], enr['enrollment_secret']

    res = pairing_service.poll(eid, esec)
    assert res['status'] == 'pending'
