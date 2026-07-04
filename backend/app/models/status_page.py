from datetime import datetime
from app import db
import json


class StatusPage(db.Model):
    """Public-facing status page configuration."""
    __tablename__ = 'status_pages'

    id = db.Column(db.Integer, primary_key=True)
    name = db.Column(db.String(128), nullable=False)
    slug = db.Column(db.String(128), nullable=False, unique=True)
    description = db.Column(db.Text)

    # Branding
    logo_url = db.Column(db.String(512))
    primary_color = db.Column(db.String(7), default='#4f46e5')
    custom_domain = db.Column(db.String(256))

    # Settings
    is_public = db.Column(db.Boolean, default=True)
    show_uptime = db.Column(db.Boolean, default=True)
    show_history = db.Column(db.Boolean, default=True)

    created_at = db.Column(db.DateTime, default=datetime.utcnow)
    updated_at = db.Column(db.DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    components = db.relationship('StatusComponent', backref='status_page', lazy='dynamic',
                                 order_by='StatusComponent.sort_order', cascade='all, delete-orphan')
    incidents = db.relationship('StatusIncident', backref='status_page', lazy='dynamic',
                                order_by='StatusIncident.created_at.desc()', cascade='all, delete-orphan')

    def to_dict(self):
        return {
            'id': self.id,
            'name': self.name,
            'slug': self.slug,
            'description': self.description,
            'logo_url': self.logo_url,
            'primary_color': self.primary_color,
            'custom_domain': self.custom_domain,
            'is_public': self.is_public,
            'show_uptime': self.show_uptime,
            'show_history': self.show_history,
            'component_count': self.components.count(),
            'created_at': self.created_at.isoformat() if self.created_at else None,
        }


class StatusComponent(db.Model):
    """A service/component shown on the status page."""
    __tablename__ = 'status_components'

    id = db.Column(db.Integer, primary_key=True)
    page_id = db.Column(db.Integer, db.ForeignKey('status_pages.id'), nullable=False)
    name = db.Column(db.String(128), nullable=False)
    description = db.Column(db.Text)
    group = db.Column(db.String(64))  # e.g., "Web Services", "APIs"
    sort_order = db.Column(db.Integer, default=0)

    # Health check config
    check_type = db.Column(db.String(16), default='http')  # http, tcp, dns, smtp, ping
    check_target = db.Column(db.String(512))  # URL, host:port, etc.
    check_interval = db.Column(db.Integer, default=60)  # seconds
    check_timeout = db.Column(db.Integer, default=10)

    # Optional binding to a managed WordPress site. When set, the component's
    # status is driven from the site's health checks (#26) rather than a network
    # probe, and a real uptime % accrues from the recorded HealthCheck rows.
    wordpress_site_id = db.Column(db.Integer, db.ForeignKey('wordpress_sites.id'), nullable=True)

    # Status
    STATUS_OPERATIONAL = 'operational'
    STATUS_DEGRADED = 'degraded'
    STATUS_PARTIAL = 'partial_outage'
    STATUS_MAJOR = 'major_outage'
    STATUS_MAINTENANCE = 'maintenance'
    status = db.Column(db.String(32), default=STATUS_OPERATIONAL)

    last_check_at = db.Column(db.DateTime)
    last_response_time = db.Column(db.Integer)  # ms

    # Uptime data
    uptime_24h = db.Column(db.Float, default=100.0)
    uptime_7d = db.Column(db.Float, default=100.0)
    uptime_30d = db.Column(db.Float, default=100.0)
    uptime_90d = db.Column(db.Float, default=100.0)

    created_at = db.Column(db.DateTime, default=datetime.utcnow)

    checks = db.relationship('HealthCheck', backref='component', lazy='dynamic',
                             order_by='HealthCheck.checked_at.desc()', cascade='all, delete-orphan')

    def to_dict(self):
        return {
            'id': self.id,
            'page_id': self.page_id,
            'name': self.name,
            'description': self.description,
            'group': self.group,
            'sort_order': self.sort_order,
            'check_type': self.check_type,
            'check_target': self.check_target,
            'check_interval': self.check_interval,
            'check_timeout': self.check_timeout,
            'wordpress_site_id': self.wordpress_site_id,
            'status': self.status,
            'last_check_at': self.last_check_at.isoformat() if self.last_check_at else None,
            'last_response_time': self.last_response_time,
            'uptime_24h': self.uptime_24h,
            'uptime_7d': self.uptime_7d,
            'uptime_30d': self.uptime_30d,
            'uptime_90d': self.uptime_90d,
        }


class HealthCheck(db.Model):
    """Individual health check result."""
    __tablename__ = 'health_checks'

    id = db.Column(db.Integer, primary_key=True)
    component_id = db.Column(db.Integer, db.ForeignKey('status_components.id'), nullable=False)
    status = db.Column(db.String(16))  # up, down, degraded
    response_time = db.Column(db.Integer)  # ms
    status_code = db.Column(db.Integer)
    error = db.Column(db.Text)
    checked_at = db.Column(db.DateTime, default=datetime.utcnow)

    def to_dict(self):
        return {
            'id': self.id,
            'component_id': self.component_id,
            'status': self.status,
            'response_time': self.response_time,
            'status_code': self.status_code,
            'error': self.error,
            'checked_at': self.checked_at.isoformat() if self.checked_at else None,
        }


class StatusIncident(db.Model):
    """An incident on the status page."""
    __tablename__ = 'status_incidents'

    id = db.Column(db.Integer, primary_key=True)
    page_id = db.Column(db.Integer, db.ForeignKey('status_pages.id'), nullable=False)
    # Optional link to the component this incident is about. Set when an incident
    # is auto-opened from a health check, so it can be auto-resolved on recovery (#26).
    component_id = db.Column(db.Integer, db.ForeignKey('status_components.id'), nullable=True)
    title = db.Column(db.String(256), nullable=False)
    status = db.Column(db.String(32), default='investigating')  # investigating, identified, monitoring, resolved
    impact = db.Column(db.String(32), default='minor')  # none, minor, major, critical
    body = db.Column(db.Text)

    # Maintenance window
    is_maintenance = db.Column(db.Boolean, default=False)
    scheduled_start = db.Column(db.DateTime)
    scheduled_end = db.Column(db.DateTime)

    resolved_at = db.Column(db.DateTime)
    created_at = db.Column(db.DateTime, default=datetime.utcnow)
    updated_at = db.Column(db.DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    updates = db.relationship('StatusIncidentUpdate', backref='incident', lazy='dynamic',
                              order_by='StatusIncidentUpdate.created_at.desc()', cascade='all, delete-orphan')

    def to_dict(self):
        return {
            'id': self.id,
            'page_id': self.page_id,
            'component_id': self.component_id,
            'title': self.title,
            'status': self.status,
            'impact': self.impact,
            'body': self.body,
            'is_maintenance': self.is_maintenance,
            'scheduled_start': self.scheduled_start.isoformat() if self.scheduled_start else None,
            'scheduled_end': self.scheduled_end.isoformat() if self.scheduled_end else None,
            'resolved_at': self.resolved_at.isoformat() if self.resolved_at else None,
            'created_at': self.created_at.isoformat() if self.created_at else None,
            'updates': [u.to_dict() for u in self.updates.limit(20).all()],
        }


class StatusIncidentUpdate(db.Model):
    """Timeline update for an incident."""
    __tablename__ = 'status_incident_updates'

    id = db.Column(db.Integer, primary_key=True)
    incident_id = db.Column(db.Integer, db.ForeignKey('status_incidents.id'), nullable=False)
    status = db.Column(db.String(32))
    body = db.Column(db.Text, nullable=False)
    created_at = db.Column(db.DateTime, default=datetime.utcnow)

    def to_dict(self):
        return {
            'id': self.id,
            'incident_id': self.incident_id,
            'status': self.status,
            'body': self.body,
            'created_at': self.created_at.isoformat() if self.created_at else None,
        }
