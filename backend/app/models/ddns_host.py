from datetime import datetime
import secrets
from app import db


class DdnsHost(db.Model):
    """A dynamic-DNS binding: a hostname whose A/AAAA record is updated on
    demand through a secret token — the URL a home router or cron job calls
    whenever its public IP changes."""
    __tablename__ = 'ddns_hosts'

    id = db.Column(db.Integer, primary_key=True)
    zone_id = db.Column(db.Integer, db.ForeignKey('dns_zones.id'), nullable=False)
    # Record name within the zone: '@' for the apex, or e.g. 'home' for
    # home.example.com. Matches DNSRecord.name.
    record_name = db.Column(db.String(256), nullable=False, default='@')
    label = db.Column(db.String(128))
    token = db.Column(db.String(64), nullable=False, unique=True, index=True)

    last_ip = db.Column(db.String(45))            # last IP applied (IPv4 or IPv6)
    last_update_at = db.Column(db.DateTime)
    enabled = db.Column(db.Boolean, default=True, nullable=False)

    created_at = db.Column(db.DateTime, default=datetime.utcnow)

    zone = db.relationship('DNSZone')

    @staticmethod
    def generate_token():
        return secrets.token_urlsafe(32)

    @property
    def hostname(self):
        domain = self.zone.domain if self.zone else ''
        if self.record_name in ('@', '', None):
            return domain
        return f'{self.record_name}.{domain}'

    def to_dict(self, include_token=False):
        data = {
            'id': self.id,
            'zone_id': self.zone_id,
            'record_name': self.record_name,
            'label': self.label,
            'hostname': self.hostname,
            'last_ip': self.last_ip,
            'last_update_at': self.last_update_at.isoformat() if self.last_update_at else None,
            'enabled': self.enabled,
            'created_at': self.created_at.isoformat() if self.created_at else None,
        }
        # The token is a credential — only return it when it was just minted.
        if include_token:
            data['token'] = self.token
        return data
