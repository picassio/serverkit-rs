from datetime import datetime
from app import db


class DomainRegistration(db.Model):
    """Cached domain registration facts (expiry, registrar) looked up lazily via
    RDAP/WHOIS. Persisted so the Domains list can show expiry without re-querying on
    every page load — keyed by bare domain so it works whether or not the domain is
    adopted as a DNS zone or attached to an app."""
    __tablename__ = 'domain_registrations'

    id = db.Column(db.Integer, primary_key=True)
    domain = db.Column(db.String(256), nullable=False, unique=True, index=True)
    expires_at = db.Column(db.DateTime, nullable=True)
    registrar = db.Column(db.String(255), nullable=True)
    auto_renew = db.Column(db.Boolean, nullable=True)
    source = db.Column(db.String(32), nullable=True)  # rdap | provider
    checked_at = db.Column(db.DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    def to_dict(self):
        return {
            'domain': self.domain,
            'expires_at': self.expires_at.isoformat() if self.expires_at else None,
            'registrar': self.registrar,
            'auto_renew': self.auto_renew,
            'source': self.source,
            'checked_at': self.checked_at.isoformat() if self.checked_at else None,
        }
