import logging
import socket
import time
from datetime import datetime, timedelta
from app import db
from app.models.status_page import (
    StatusPage, StatusComponent, HealthCheck, StatusIncident, StatusIncidentUpdate
)
from app.utils.slug import validate_slug

logger = logging.getLogger(__name__)


class StatusPageService:
    """Service for public status pages and automated health checks."""

    @staticmethod
    def normalize_slug(value):
        return validate_slug(value)

    # --- Pages ---

    @staticmethod
    def list_pages():
        return StatusPage.query.order_by(StatusPage.name).all()

    @staticmethod
    def get_page(page_id):
        return StatusPage.query.get(page_id)

    @staticmethod
    def get_page_by_slug(slug):
        return StatusPage.query.filter_by(slug=slug).first()

    @staticmethod
    def create_page(data):
        slug = StatusPageService.normalize_slug(data.get('slug'))
        if StatusPage.query.filter_by(slug=slug).first():
            raise ValueError(f"Status page '{slug}' already exists")

        page = StatusPage(
            name=data['name'],
            slug=slug,
            description=data.get('description', ''),
            logo_url=data.get('logo_url'),
            primary_color=data.get('primary_color', '#4f46e5'),
            custom_domain=data.get('custom_domain'),
            is_public=data.get('is_public', True),
            show_uptime=data.get('show_uptime', True),
            show_history=data.get('show_history', True),
        )
        db.session.add(page)
        db.session.commit()
        return page

    @staticmethod
    def update_page(page_id, data):
        page = StatusPage.query.get(page_id)
        if not page:
            return None
        for field in ['name', 'description', 'logo_url', 'primary_color',
                      'custom_domain', 'is_public', 'show_uptime', 'show_history']:
            if field in data:
                setattr(page, field, data[field])
        db.session.commit()
        return page

    @staticmethod
    def delete_page(page_id):
        page = StatusPage.query.get(page_id)
        if not page:
            return False
        db.session.delete(page)
        db.session.commit()
        return True

    @staticmethod
    def get_public_page(slug):
        """Get public status page data (no auth required)."""
        page = StatusPage.query.filter_by(slug=slug, is_public=True).first()
        if not page:
            return None

        # Internal probe config must not appear on the unauthenticated public page
        # (a health-driven WP component may carry an internal localhost:port target).
        public_hidden = ('check_type', 'check_target', 'check_interval', 'check_timeout')
        components = page.components.all()
        grouped = {}
        for comp in components:
            group = comp.group or 'Services'
            cd = comp.to_dict()
            for k in public_hidden:
                cd.pop(k, None)
            grouped.setdefault(group, []).append(cd)

        # Active incidents
        active_incidents = page.incidents.filter(
            StatusIncident.status != 'resolved'
        ).limit(10).all()

        # Recent resolved
        resolved = page.incidents.filter_by(status='resolved').limit(5).all()

        # Overall status
        statuses = [c.status for c in components]
        if any(s == 'major_outage' for s in statuses):
            overall = 'major_outage'
        elif any(s in ('partial_outage', 'degraded') for s in statuses):
            overall = 'degraded'
        elif any(s == 'maintenance' for s in statuses):
            overall = 'maintenance'
        else:
            overall = 'operational'

        return {
            'page': page.to_dict(),
            'overall_status': overall,
            'groups': grouped,
            'active_incidents': [i.to_dict() for i in active_incidents],
            'recent_incidents': [i.to_dict() for i in resolved],
        }

    # --- Components ---

    @staticmethod
    def create_component(page_id, data):
        page = StatusPage.query.get(page_id)
        if not page:
            raise ValueError('Status page not found')

        comp = StatusComponent(
            page_id=page_id,
            name=data['name'],
            description=data.get('description', ''),
            group=data.get('group', 'Services'),
            sort_order=data.get('sort_order', 0),
            check_type=data.get('check_type', 'http'),
            check_target=data.get('check_target', ''),
            check_interval=data.get('check_interval', 60),
            check_timeout=data.get('check_timeout', 10),
            wordpress_site_id=data.get('wordpress_site_id'),
        )
        db.session.add(comp)
        db.session.commit()
        return comp

    @staticmethod
    def update_component(comp_id, data):
        comp = StatusComponent.query.get(comp_id)
        if not comp:
            return None
        for field in ['name', 'description', 'group', 'sort_order', 'check_type',
                      'check_target', 'check_interval', 'check_timeout', 'status']:
            if field in data:
                setattr(comp, field, data[field])
        db.session.commit()
        return comp

    @staticmethod
    def delete_component(comp_id):
        comp = StatusComponent.query.get(comp_id)
        if not comp:
            return False
        # Resolve + unlink any incidents referencing this component first, so we
        # never dangle the component_id FK (enforced on PostgreSQL) or leave a
        # stale active incident on the public page after the component is gone.
        for inc in StatusIncident.query.filter_by(component_id=comp_id).all():
            if inc.status != 'resolved':
                inc.status = 'resolved'
                inc.resolved_at = datetime.utcnow()
            inc.component_id = None
        db.session.delete(comp)
        db.session.commit()
        return True

    # --- Health Checks ---

    @staticmethod
    def run_check(component_id):
        """Run a health check for a component."""
        comp = StatusComponent.query.get(component_id)
        if not comp:
            return None

        check_result = StatusPageService._perform_check(comp)

        hc = HealthCheck(
            component_id=component_id,
            status=check_result['status'],
            response_time=check_result.get('response_time'),
            status_code=check_result.get('status_code'),
            error=check_result.get('error'),
        )
        db.session.add(hc)

        # Update component
        comp.last_check_at = datetime.utcnow()
        comp.last_response_time = check_result.get('response_time')
        if check_result['status'] == 'up':
            comp.status = StatusComponent.STATUS_OPERATIONAL
        elif check_result['status'] == 'degraded':
            comp.status = StatusComponent.STATUS_DEGRADED
        else:
            comp.status = StatusComponent.STATUS_MAJOR

        db.session.commit()
        return hc

    @staticmethod
    def _perform_check(comp):
        """Execute the actual health check."""
        start = time.time()
        result = {'status': 'down', 'response_time': None, 'error': None}

        try:
            if comp.check_type == 'http':
                import requests
                resp = requests.get(comp.check_target, timeout=comp.check_timeout, verify=True)
                result['response_time'] = int((time.time() - start) * 1000)
                result['status_code'] = resp.status_code
                if resp.status_code < 400:
                    result['status'] = 'up'
                elif resp.status_code < 500:
                    result['status'] = 'degraded'
                else:
                    result['status'] = 'down'

            elif comp.check_type == 'tcp':
                host, port = comp.check_target.rsplit(':', 1)
                sock = socket.create_connection((host, int(port)), timeout=comp.check_timeout)
                result['response_time'] = int((time.time() - start) * 1000)
                result['status'] = 'up'
                sock.close()

            elif comp.check_type == 'ping':
                from app.utils.system import run_command
                res = run_command(['ping', '-c', '1', '-W', str(comp.check_timeout), comp.check_target])
                result['response_time'] = int((time.time() - start) * 1000)
                result['status'] = 'up'

            elif comp.check_type == 'dns':
                socket.getaddrinfo(comp.check_target, None)
                result['response_time'] = int((time.time() - start) * 1000)
                result['status'] = 'up'

        except Exception as e:
            result['response_time'] = int((time.time() - start) * 1000)
            result['error'] = str(e)

        return result

    @staticmethod
    def get_check_history(component_id, hours=24):
        since = datetime.utcnow() - timedelta(hours=hours)
        return HealthCheck.query.filter(
            HealthCheck.component_id == component_id,
            HealthCheck.checked_at >= since
        ).order_by(HealthCheck.checked_at.desc()).all()

    @staticmethod
    def recompute_uptime(comp):
        """Recompute uptime_24h/7d/30d/90d for a component from its HealthCheck
        rows (fraction of recorded checks with status 'up'). Only fully-healthy
        checks count as up — 'degraded' periods reduce the percentage, matching
        the status-page convention where degraded is not "operational". Leaves a
        window's existing value untouched when it has no samples yet."""
        windows = {'uptime_24h': 24, 'uptime_7d': 24 * 7,
                   'uptime_30d': 24 * 30, 'uptime_90d': 24 * 90}
        now = datetime.utcnow()
        for field, hours in windows.items():
            since = now - timedelta(hours=hours)
            base = HealthCheck.query.filter(
                HealthCheck.component_id == comp.id,
                HealthCheck.checked_at >= since,
            )
            total = base.count()
            if total:
                up = base.filter(HealthCheck.status == 'up').count()
                setattr(comp, field, round(up / total * 100, 2))
        db.session.commit()

    # Map an EnvironmentHealthService overall_status to (HealthCheck status,
    # StatusComponent status). 'unknown' is intentionally absent — indeterminate
    # checks are not recorded so they don't pollute the uptime %.
    _HEALTH_MAP = {
        'healthy': ('up', StatusComponent.STATUS_OPERATIONAL),
        'degraded': ('degraded', StatusComponent.STATUS_DEGRADED),
        'unhealthy': ('down', StatusComponent.STATUS_MAJOR),
    }

    @staticmethod
    def sync_component_from_health(comp, overall_status, error=None):
        """Drive a managed-site-bound component from an EnvironmentHealthService
        verdict instead of a network probe (#26): record a HealthCheck sample,
        update the component's live status, recompute uptime, and auto-open /
        auto-resolve an incident on the operational<->major_outage edge.

        Returns the recorded HealthCheck, or None for an indeterminate
        ('unknown') verdict (not recorded).
        """
        mapped = StatusPageService._HEALTH_MAP.get(overall_status)
        if not mapped:
            return None
        check_status, comp_status = mapped
        prev_status = comp.status

        hc = HealthCheck(component_id=comp.id, status=check_status, error=error)
        db.session.add(hc)
        comp.last_check_at = datetime.utcnow()
        comp.status = comp_status
        db.session.commit()

        StatusPageService.recompute_uptime(comp)

        # Open an incident when ENTERING a major outage; resolve it when LEAVING
        # major (to operational OR degraded). Resolving on the leaving-edge — not
        # only on a clean major->operational hop — ensures a recovery that passes
        # through an intermediate degraded poll (a common path) never leaves the
        # incident stuck open. Degraded itself never opens a full incident.
        if comp_status == StatusComponent.STATUS_MAJOR and prev_status != StatusComponent.STATUS_MAJOR:
            StatusPageService._open_incident_for_component(comp, error)
        elif comp_status != StatusComponent.STATUS_MAJOR and prev_status == StatusComponent.STATUS_MAJOR:
            StatusPageService._resolve_incident_for_component(comp)
        return hc

    @staticmethod
    def _open_incident_for_component(comp, error=None):
        """Open a major-impact incident for a component if one is not already open."""
        existing = StatusIncident.query.filter(
            StatusIncident.component_id == comp.id,
            StatusIncident.status != 'resolved',
        ).first()
        if existing:
            return existing
        incident = StatusPageService.create_incident(comp.page_id, {
            'title': f'{comp.name} is experiencing an outage',
            'status': 'investigating',
            'impact': 'major',
            'body': error or 'Automated health check detected an outage.',
        })
        incident.component_id = comp.id
        db.session.commit()
        return incident

    @staticmethod
    def _resolve_incident_for_component(comp):
        """Resolve the open auto-incident for a component, if any."""
        existing = StatusIncident.query.filter(
            StatusIncident.component_id == comp.id,
            StatusIncident.status != 'resolved',
        ).first()
        if existing:
            StatusPageService.update_incident(existing.id, {
                'status': 'resolved',
                'update_body': 'Automated health check detected recovery.',
            })

    # --- Incidents ---

    @staticmethod
    def create_incident(page_id, data):
        incident = StatusIncident(
            page_id=page_id,
            title=data['title'],
            status=data.get('status', 'investigating'),
            impact=data.get('impact', 'minor'),
            body=data.get('body', ''),
            is_maintenance=data.get('is_maintenance', False),
            scheduled_start=data.get('scheduled_start'),
            scheduled_end=data.get('scheduled_end'),
        )
        db.session.add(incident)
        db.session.commit()
        return incident

    @staticmethod
    def update_incident(incident_id, data):
        incident = StatusIncident.query.get(incident_id)
        if not incident:
            return None
        for field in ['title', 'status', 'impact', 'body']:
            if field in data:
                setattr(incident, field, data[field])
        if data.get('status') == 'resolved':
            incident.resolved_at = datetime.utcnow()

        # Add timeline update
        if data.get('update_body'):
            update = StatusIncidentUpdate(
                incident_id=incident_id,
                status=data.get('status', incident.status),
                body=data['update_body'],
            )
            db.session.add(update)

        db.session.commit()
        return incident

    @staticmethod
    def delete_incident(incident_id):
        incident = StatusIncident.query.get(incident_id)
        if not incident:
            return False
        db.session.delete(incident)
        db.session.commit()
        return True

    @staticmethod
    def get_badge(slug):
        """Generate status badge data."""
        page = StatusPage.query.filter_by(slug=slug).first()
        if not page:
            return None

        components = page.components.all()
        statuses = [c.status for c in components]

        if not statuses or all(s == 'operational' for s in statuses):
            return {'label': 'status', 'message': 'operational', 'color': 'brightgreen'}
        elif any(s == 'major_outage' for s in statuses):
            return {'label': 'status', 'message': 'major outage', 'color': 'red'}
        elif any(s in ('partial_outage', 'degraded') for s in statuses):
            return {'label': 'status', 'message': 'degraded', 'color': 'yellow'}
        else:
            return {'label': 'status', 'message': 'maintenance', 'color': 'blue'}
