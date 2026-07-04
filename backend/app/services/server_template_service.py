import json
import logging
from datetime import datetime
from app import db
from app.models.server_template import ServerTemplate, ServerTemplateAssignment
from app.models.server import Server

logger = logging.getLogger(__name__)


class ServerTemplateService:
    """Service for server template management and config drift detection."""

    # Built-in template library
    TEMPLATE_LIBRARY = {
        'web-server': {
            'name': 'Web Server',
            'description': 'Nginx + PHP-FPM web server with standard security',
            'category': 'web',
            'packages': ['nginx', 'php-fpm', 'certbot'],
            'services': [
                {'name': 'nginx', 'enabled': True, 'running': True},
                {'name': 'php-fpm', 'enabled': True, 'running': True},
            ],
            'firewall_rules': [
                {'port': 80, 'protocol': 'tcp', 'action': 'allow'},
                {'port': 443, 'protocol': 'tcp', 'action': 'allow'},
                {'port': 22, 'protocol': 'tcp', 'action': 'allow'},
            ],
        },
        'database-server': {
            'name': 'Database Server',
            'description': 'MySQL/MariaDB database server with backups',
            'category': 'database',
            'packages': ['mariadb-server'],
            'services': [
                {'name': 'mariadb', 'enabled': True, 'running': True},
            ],
            'firewall_rules': [
                {'port': 3306, 'protocol': 'tcp', 'action': 'allow'},
                {'port': 22, 'protocol': 'tcp', 'action': 'allow'},
            ],
        },
        'mail-server': {
            'name': 'Mail Server',
            'description': 'Postfix + Dovecot mail server',
            'category': 'mail',
            'packages': ['postfix', 'dovecot-imapd', 'dovecot-pop3d', 'spamassassin', 'opendkim'],
            'services': [
                {'name': 'postfix', 'enabled': True, 'running': True},
                {'name': 'dovecot', 'enabled': True, 'running': True},
                {'name': 'spamassassin', 'enabled': True, 'running': True},
            ],
            'firewall_rules': [
                {'port': 25, 'protocol': 'tcp', 'action': 'allow'},
                {'port': 587, 'protocol': 'tcp', 'action': 'allow'},
                {'port': 993, 'protocol': 'tcp', 'action': 'allow'},
                {'port': 995, 'protocol': 'tcp', 'action': 'allow'},
                {'port': 22, 'protocol': 'tcp', 'action': 'allow'},
            ],
        },

        # ───── one-click stacks (Phase 4) ─────────────────────────────
        # Curated entries that the agent's packages:* + systemd:* surface
        # can apply unattended. Package names target Debian/Ubuntu where
        # they differ across distros — RHEL/Alpine users will need to
        # tweak names. The list here drives the workflow engine's
        # capability_gate + agent_command nodes; the agent makes the
        # apt/dnf/apk distinction at install time.
        'lemp': {
            'name': 'LEMP Stack',
            'description': 'Nginx + MariaDB + PHP-FPM (Linux/Nginx/MariaDB/PHP)',
            'category': 'web',
            'packages': ['nginx', 'mariadb-server', 'php-fpm', 'php-mysql'],
            'services': [
                {'name': 'nginx', 'enabled': True, 'running': True},
                {'name': 'mariadb', 'enabled': True, 'running': True},
                {'name': 'php-fpm', 'enabled': True, 'running': True},
            ],
            'firewall_rules': [
                {'port': 80, 'protocol': 'tcp', 'action': 'allow'},
                {'port': 443, 'protocol': 'tcp', 'action': 'allow'},
                {'port': 22, 'protocol': 'tcp', 'action': 'allow'},
            ],
        },
        'lamp': {
            'name': 'LAMP Stack',
            'description': 'Apache + MariaDB + PHP (mod_php)',
            'category': 'web',
            'packages': ['apache2', 'mariadb-server', 'php', 'libapache2-mod-php', 'php-mysql'],
            'services': [
                {'name': 'apache2', 'enabled': True, 'running': True},
                {'name': 'mariadb', 'enabled': True, 'running': True},
            ],
            'firewall_rules': [
                {'port': 80, 'protocol': 'tcp', 'action': 'allow'},
                {'port': 443, 'protocol': 'tcp', 'action': 'allow'},
                {'port': 22, 'protocol': 'tcp', 'action': 'allow'},
            ],
        },
        'docker-host': {
            'name': 'Docker Host',
            'description': 'Docker Engine + Compose plugin',
            'category': 'container',
            # docker.io is the Debian/Ubuntu package; users on RHEL/Fedora
            # should swap to docker-ce via the Docker repo. The agent
            # surfaces a clear "no candidate" error on those distros so
            # the failure mode is explicit, not silent.
            'packages': ['docker.io', 'docker-compose-plugin'],
            'services': [
                {'name': 'docker', 'enabled': True, 'running': True},
            ],
            'firewall_rules': [
                {'port': 22, 'protocol': 'tcp', 'action': 'allow'},
            ],
        },
        'node-host': {
            'name': 'Node.js Host',
            'description': 'Node.js + npm runtime (no service — start your apps via systemd or pm2)',
            'category': 'runtime',
            'packages': ['nodejs', 'npm'],
            'services': [],
            'firewall_rules': [
                {'port': 22, 'protocol': 'tcp', 'action': 'allow'},
            ],
        },
        'python-host': {
            'name': 'Python Host',
            'description': 'Python 3 + venv + pip + Gunicorn',
            'category': 'runtime',
            'packages': ['python3', 'python3-venv', 'python3-pip', 'gunicorn'],
            'services': [],
            'firewall_rules': [
                {'port': 22, 'protocol': 'tcp', 'action': 'allow'},
            ],
        },
        'redis': {
            'name': 'Redis',
            'description': 'Redis in-memory data store',
            'category': 'cache',
            'packages': ['redis-server'],
            'services': [
                {'name': 'redis-server', 'enabled': True, 'running': True},
            ],
            'firewall_rules': [
                {'port': 6379, 'protocol': 'tcp', 'action': 'allow'},
                {'port': 22, 'protocol': 'tcp', 'action': 'allow'},
            ],
        },
        'postgres': {
            'name': 'PostgreSQL',
            'description': 'PostgreSQL relational database',
            'category': 'database',
            'packages': ['postgresql'],
            'services': [
                {'name': 'postgresql', 'enabled': True, 'running': True},
            ],
            'firewall_rules': [
                {'port': 5432, 'protocol': 'tcp', 'action': 'allow'},
                {'port': 22, 'protocol': 'tcp', 'action': 'allow'},
            ],
        },
        'cloudflared': {
            'name': 'Cloudflare Tunnel',
            'description': 'cloudflared agent for outbound-only Cloudflare named tunnels',
            'category': 'network',
            'packages': ['cloudflared'],
            'services': [
                {'name': 'cloudflared', 'enabled': True, 'running': False},
            ],
            'firewall_rules': [
                {'port': 22, 'protocol': 'tcp', 'action': 'allow'},
            ],
        },
        'fail2ban': {
            'name': 'Fail2ban',
            'description': 'Brute-force protection / log-driven IP banning',
            'category': 'security',
            'packages': ['fail2ban'],
            'services': [
                {'name': 'fail2ban', 'enabled': True, 'running': True},
            ],
        },
    }

    @staticmethod
    def list_templates(category=None):
        query = ServerTemplate.query
        if category:
            query = query.filter_by(category=category)
        return query.order_by(ServerTemplate.name).all()

    @staticmethod
    def get_template(template_id):
        return ServerTemplate.query.get(template_id)

    @staticmethod
    def create_template(data, user_id=None):
        if ServerTemplate.query.filter_by(name=data['name']).first():
            raise ValueError(f"Template '{data['name']}' already exists")

        template = ServerTemplate(
            name=data['name'],
            description=data.get('description', ''),
            category=data.get('category', 'general'),
            parent_id=data.get('parent_id'),
            auto_remediate=data.get('auto_remediate', False),
            remediation_approval_required=data.get('remediation_approval_required', True),
            created_by=user_id,
        )
        template.packages = data.get('packages', [])
        template.services = data.get('services', [])
        template.firewall_rules = data.get('firewall_rules', [])
        template.files = data.get('files', [])
        template.users = data.get('users', [])
        template.sysctl_params = data.get('sysctl_params', [])

        db.session.add(template)
        db.session.commit()
        return template

    @staticmethod
    def update_template(template_id, data):
        template = ServerTemplate.query.get(template_id)
        if not template:
            return None

        for field in ['name', 'description', 'category', 'parent_id',
                      'auto_remediate', 'remediation_approval_required']:
            if field in data:
                setattr(template, field, data[field])

        for json_field in ['packages', 'services', 'firewall_rules', 'files', 'users', 'sysctl_params']:
            if json_field in data:
                setattr(template, json_field, data[json_field])

        template.version += 1
        db.session.commit()
        return template

    @staticmethod
    def delete_template(template_id):
        template = ServerTemplate.query.get(template_id)
        if not template:
            return False
        active = template.assignments.count()
        if active > 0:
            raise ValueError(f'Cannot delete template with {active} active assignments')
        # Remove children references
        for child in template.children:
            child.parent_id = None
        db.session.delete(template)
        db.session.commit()
        return True

    @staticmethod
    def get_library_templates():
        return ServerTemplateService.TEMPLATE_LIBRARY

    @staticmethod
    def create_from_library(key, user_id=None):
        if key not in ServerTemplateService.TEMPLATE_LIBRARY:
            raise ValueError(f"Unknown library template: {key}")
        data = ServerTemplateService.TEMPLATE_LIBRARY[key].copy()
        return ServerTemplateService.create_template(data, user_id)

    # --- Assignment & Drift ---

    @staticmethod
    def assign_template(template_id, server_id):
        template = ServerTemplate.query.get(template_id)
        if not template:
            raise ValueError('Template not found')
        server = Server.query.get(server_id)
        if not server:
            raise ValueError('Server not found')

        existing = ServerTemplateAssignment.query.filter_by(
            template_id=template_id, server_id=server_id
        ).first()
        if existing:
            raise ValueError('Template already assigned to this server')

        assignment = ServerTemplateAssignment(
            template_id=template_id,
            server_id=server_id,
        )
        db.session.add(assignment)
        db.session.commit()
        return assignment

    @staticmethod
    def unassign_template(assignment_id):
        assignment = ServerTemplateAssignment.query.get(assignment_id)
        if not assignment:
            return False
        db.session.delete(assignment)
        db.session.commit()
        return True

    @staticmethod
    def bulk_assign(template_id, server_ids):
        results = []
        for sid in server_ids:
            try:
                a = ServerTemplateService.assign_template(template_id, sid)
                results.append({'server_id': sid, 'status': 'assigned', 'assignment_id': a.id})
            except ValueError as e:
                results.append({'server_id': sid, 'status': 'error', 'error': str(e)})
        return results

    @staticmethod
    def check_drift(assignment_id):
        """Check configuration drift for a server assignment."""
        assignment = ServerTemplateAssignment.query.get(assignment_id)
        if not assignment:
            return None

        # Agent-side config drift checking is NOT implemented (the agent has no
        # config_drift_check handler, and the old code dispatched to a
        # non-existent get_agent_gateway()). Report honestly instead of leaving
        # the assignment stuck in 'checking' forever.
        assignment.status = ServerTemplateAssignment.STATUS_UNKNOWN
        assignment.drift_report = {
            'error': 'Agent-side configuration drift checking is not implemented yet',
        }
        assignment.last_check_at = datetime.utcnow()
        db.session.commit()

        return assignment

    @staticmethod
    def update_drift_report(assignment_id, report):
        assignment = ServerTemplateAssignment.query.get(assignment_id)
        if not assignment:
            return None
        assignment.drift_report = report
        assignment.last_check_at = datetime.utcnow()
        has_drift = any(
            report.get(k, []) for k in ['missing_packages', 'extra_packages',
                                          'stopped_services', 'missing_rules', 'changed_files']
        )
        assignment.status = (
            ServerTemplateAssignment.STATUS_DRIFTED if has_drift
            else ServerTemplateAssignment.STATUS_COMPLIANT
        )
        db.session.commit()
        return assignment

    @staticmethod
    def remediate(assignment_id):
        """Apply template to bring server back to expected state."""
        assignment = ServerTemplateAssignment.query.get(assignment_id)
        if not assignment:
            return None

        # Agent-side remediation is NOT implemented (no config_remediate handler;
        # old code dispatched to a non-existent get_agent_gateway()). Report
        # honestly instead of leaving the assignment stuck in 'remediating'.
        assignment.status = ServerTemplateAssignment.STATUS_UNKNOWN
        assignment.drift_report = {
            'error': 'Agent-side template remediation is not implemented yet',
        }
        assignment.last_remediation_at = datetime.utcnow()
        db.session.commit()

        return assignment

    @staticmethod
    def get_server_assignments(server_id):
        return ServerTemplateAssignment.query.filter_by(server_id=server_id).all()

    @staticmethod
    def get_template_assignments(template_id):
        return ServerTemplateAssignment.query.filter_by(template_id=template_id).all()

    @staticmethod
    def get_compliance_summary():
        """Get fleet-wide compliance summary."""
        assignments = ServerTemplateAssignment.query.all()
        total = len(assignments)
        if total == 0:
            return {'total': 0, 'compliant': 0, 'drifted': 0, 'unknown': 0, 'compliance_pct': 100}

        compliant = sum(1 for a in assignments if a.status == 'compliant')
        drifted = sum(1 for a in assignments if a.status == 'drifted')
        unknown = total - compliant - drifted

        return {
            'total': total,
            'compliant': compliant,
            'drifted': drifted,
            'unknown': unknown,
            'compliance_pct': round(compliant / total * 100, 1) if total > 0 else 100,
        }
