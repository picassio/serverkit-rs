import json
from datetime import datetime
from decimal import Decimal

from app import db

# Every data-protection target the unified policy system can back up (§8).
# 'application' and 'wordpress_site' are the original targets; 'database',
# 'files', and 'server' were folded in from the legacy BackupService so a
# single BackupPolicy/BackupRun system covers all data-protection backups.
VALID_TARGET_TYPES = ('application', 'wordpress_site', 'database', 'files', 'server')


def _num(value):
    """Serialize a Numeric/Decimal column to a JSON-friendly float (or None)."""
    if value is None:
        return None
    if isinstance(value, Decimal):
        return float(value)
    return value


class BackupPolicy(db.Model):
    """Automated backup ("protection") policy for a single target.

    A target is either a WordPress site (``target_type='wordpress_site'``) or a
    generic application (``target_type='application'``). There is at most one
    policy per target. The cron schedule is mirrored into a ``ScheduledJob`` row
    by :class:`BackupPolicyService` so firing happens on the unified job bus.

    The ``last_*`` columns are a denormalized cache of the most recent run so the
    UI can render the protection status without scanning ``backup_runs``.
    """

    __tablename__ = 'backup_policies'

    id = db.Column(db.Integer, primary_key=True)
    target_type = db.Column(db.String(40), nullable=False, index=True)  # see VALID_TARGET_TYPES
    target_id = db.Column(db.Integer, nullable=False, index=True)
    # Optional finer target classification, e.g. the engine for a 'database'
    # target ('mysql' | 'postgresql' | 'mongodb') or 'pathlist' for 'files'.
    target_subtype = db.Column(db.String(40), nullable=True)
    # Per-target details that aren't a first-class column: for 'database' the
    # connection descriptor (db_type/db_name/user/host); for 'files' the path
    # list; for 'server' the scope. JSON object, never secrets in plaintext.
    target_meta_json = db.Column(db.Text, default='{}', nullable=True)
    enabled = db.Column(db.Boolean, default=False, nullable=False)

    # Schedule (cron expression fired by the unified scheduler)
    schedule_cron = db.Column(db.String(120), default='0 2 * * *', nullable=False)

    # Retention
    retention_count = db.Column(db.Integer, default=14, nullable=False)
    retention_days = db.Column(db.Integer, default=30, nullable=False)

    # Smart backup
    full_every_n_days = db.Column(db.Integer, default=7, nullable=False)
    compression = db.Column(db.String(20), default='balanced', nullable=False)  # 'fast' | 'balanced' | 'max'

    # Remote
    remote_copy = db.Column(db.Boolean, default=False, nullable=False)

    # Hooks (optional shell snippets run before/after a backup)
    pre_backup_hook = db.Column(db.Text, nullable=True)
    post_backup_hook = db.Column(db.Text, nullable=True)

    # Denormalized last-run cache for the UI
    last_run_at = db.Column(db.DateTime, nullable=True)
    last_status = db.Column(db.String(20), nullable=True)  # 'success' | 'failed' | 'running'
    last_size = db.Column(db.BigInteger, nullable=True)
    last_cost_local = db.Column(db.Numeric(10, 4), nullable=True)
    last_cost_remote = db.Column(db.Numeric(10, 4), nullable=True)
    last_job_id = db.Column(db.String(36), nullable=True)

    created_at = db.Column(db.DateTime, default=datetime.utcnow)
    updated_at = db.Column(db.DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    runs = db.relationship(
        'BackupRun',
        backref='policy',
        lazy='dynamic',
        cascade='all, delete-orphan',
        order_by='BackupRun.started_at.desc()',
    )

    __table_args__ = (db.UniqueConstraint('target_type', 'target_id', name='uq_backup_policy_target'),)

    def get_target_meta(self):
        """Parsed target_meta_json (always a dict)."""
        if not self.target_meta_json:
            return {}
        try:
            data = json.loads(self.target_meta_json)
            return data if isinstance(data, dict) else {}
        except (ValueError, TypeError):
            return {}

    def set_target_meta(self, value):
        self.target_meta_json = json.dumps(value or {})

    def to_dict(self):
        return {
            'id': self.id,
            'target_type': self.target_type,
            'target_id': self.target_id,
            'target_subtype': self.target_subtype,
            'target_meta': self.get_target_meta(),
            'enabled': self.enabled,
            'schedule_cron': self.schedule_cron,
            'retention_count': self.retention_count,
            'retention_days': self.retention_days,
            'full_every_n_days': self.full_every_n_days,
            'compression': self.compression,
            'remote_copy': self.remote_copy,
            'pre_backup_hook': self.pre_backup_hook,
            'post_backup_hook': self.post_backup_hook,
            'last_run_at': self.last_run_at.isoformat() if self.last_run_at else None,
            'last_status': self.last_status,
            'last_size': self.last_size,
            'last_cost_local': _num(self.last_cost_local),
            'last_cost_remote': _num(self.last_cost_remote),
            'last_job_id': self.last_job_id,
            'created_at': self.created_at.isoformat() if self.created_at else None,
            'updated_at': self.updated_at.isoformat() if self.updated_at else None,
        }

    def __repr__(self):
        return f'<BackupPolicy {self.id} {self.target_type}:{self.target_id} enabled={self.enabled}>'
