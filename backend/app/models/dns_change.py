from datetime import datetime
from app import db


class DnsChange(db.Model):
    """Audit trail of every DNS record write ServerKit sends to a connected provider.

    Powers the "Changes to your Cloudflare" activity feed on the connection — so the
    user can always see exactly what the panel created, updated, or deleted in their
    zone (and what failed). Written at the single write choke point
    (DnsOwnershipService.guarded_upsert / guarded_delete), so coverage is complete.
    """
    __tablename__ = 'dns_changes'

    id = db.Column(db.Integer, primary_key=True)
    dns_provider_config_id = db.Column(
        db.Integer, db.ForeignKey('dns_provider_configs.id'), nullable=True, index=True)
    provider = db.Column(db.String(64), nullable=False)
    provider_zone_id = db.Column(db.String(128), index=True)
    action = db.Column(db.String(16), nullable=False)        # create | update | delete
    record_type = db.Column(db.String(10))
    name = db.Column(db.String(256))
    content = db.Column(db.Text)                             # value we set (after)
    provider_record_id = db.Column(db.String(128))
    source = db.Column(db.String(40))                        # zone|ddns|wordpress|email|auto-dns|caa|...
    result = db.Column(db.String(16), nullable=False)        # ok | error | conflict | skipped
    error = db.Column(db.Text)

    created_at = db.Column(db.DateTime, default=datetime.utcnow, index=True)

    def to_dict(self):
        return {
            'id': self.id,
            'dns_provider_config_id': self.dns_provider_config_id,
            'provider': self.provider,
            'provider_zone_id': self.provider_zone_id,
            'action': self.action,
            'record_type': self.record_type,
            'name': self.name,
            'content': self.content,
            'provider_record_id': self.provider_record_id,
            'source': self.source,
            'result': self.result,
            'error': self.error,
            'created_at': self.created_at.isoformat() if self.created_at else None,
        }
