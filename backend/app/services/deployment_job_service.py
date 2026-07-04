"""Deployment job orchestration service."""

import os
import uuid
from datetime import datetime
from typing import Dict, Optional

from app import db
from app.models import Application, Server
from app.models.deployment_job import DeploymentJob
from app.services.deployment_runner import DeploymentPlanRunner
from app.services.docker_service import DockerService
from app.services.template_service import TemplateService
from app.services.telemetry_service import generate_correlation_id

# Unified job kind for asynchronous template installs (see register_jobs()).
JOB_KIND = 'deploy.install'


class DeploymentJobService:
    """Creates and runs deployment jobs with persistent logs."""

    @classmethod
    def install_template(
        cls,
        template_id: str,
        app_name: str,
        user_variables: Dict = None,
        user_id: int = None,
        server_id: Optional[str] = None,
        wait: bool = False,
    ) -> Dict:
        """Create a template installation job and optionally run it synchronously."""
        normalized_server_id = cls._normalize_server_id(server_id)

        existing = Application.query.filter_by(name=app_name, server_id=normalized_server_id).first()
        if existing:
            return {
                'success': False,
                'error': f'An application named "{app_name}" already exists on this target server'
            }

        if normalized_server_id:
            server = Server.query.get(normalized_server_id)
            if not server:
                return {'success': False, 'error': 'Target server not found'}

        plan_result = TemplateService.build_install_plan(
            template_id=template_id,
            app_name=app_name,
            user_variables=user_variables or {},
            user_id=user_id,
            server_id=normalized_server_id,
        )
        if not plan_result.get('success'):
            return plan_result

        app_path = plan_result['app_path']
        if not normalized_server_id and os.path.exists(app_path):
            return {'success': False, 'error': f"App directory already exists: {app_path}"}

        job = DeploymentJob(
            id=str(uuid.uuid4()),
            kind='template_install',
            status='pending',
            target_server_id=normalized_server_id,
            requested_by=user_id,
            trigger='manual',
            correlation_id=generate_correlation_id(),
        )
        job.set_plan(plan_result['plan'])
        db.session.add(job)
        db.session.commit()

        if wait:
            cls.run_job(job.id)
        else:
            cls._enqueue_install(job)

        return {
            'success': True,
            'job_id': job.id,
            'job': job.to_dict(include_logs=True),
        }

    @classmethod
    def run_job(cls, job_id: str) -> Dict:
        """Run a job by ID."""
        job = DeploymentJob.query.get(job_id)
        if not job:
            return {'success': False, 'error': 'Deployment job not found'}

        if job.kind != 'template_install':
            return {'success': False, 'error': f'Unsupported deployment job kind: {job.kind}'}

        runner = DeploymentPlanRunner(job)
        run_result = runner.run()

        if not run_result.get('success'):
            return run_result

        try:
            return cls._finalize_template_install(job)
        except Exception as exc:
            job.status = 'failed'
            job.error_message = str(exc)
            job.completed_at = datetime.utcnow()
            db.session.commit()
            runner.log('error', f'Failed to finalize deployment: {exc}')
            return {'success': False, 'error': str(exc)}

    @classmethod
    def get_job(cls, job_id: str, include_logs: bool = True) -> Optional[Dict]:
        job = DeploymentJob.query.get(job_id)
        return job.to_dict(include_logs=include_logs) if job else None

    @classmethod
    def list_jobs(cls, status: str = None, target_server_id: str = None, limit: int = 50):
        query = DeploymentJob.query.order_by(DeploymentJob.created_at.desc())
        if status:
            query = query.filter_by(status=status)
        if target_server_id:
            query = query.filter_by(target_server_id=cls._normalize_server_id(target_server_id))
        return [job.to_dict() for job in query.limit(limit).all()]

    @classmethod
    def _finalize_template_install(cls, job: DeploymentJob) -> Dict:
        plan = job.get_plan()
        app_name = plan.get('app_name')
        app_path = plan.get('app_path')
        app_port = plan.get('port')
        template_name = plan.get('template_name')

        app = Application(
            name=app_name,
            app_type='docker',
            status='running',
            root_path=app_path,
            docker_image=template_name,
            user_id=job.requested_by or 1,
            port=app_port,
            server_id=job.target_server_id,
        )
        db.session.add(app)
        db.session.commit()

        port_accessible = None
        if not job.target_server_id and app_port:
            port_accessible = DockerService.check_port_accessible(app_port).get('accessible', False)

        config = TemplateService.get_config()
        config.setdefault('installed', {})[str(app.id)] = {
            'template_id': plan.get('template_id'),
            'template_version': plan.get('template_version'),
            'app_id': app.id,
            'app_name': app_name,
            'server_id': job.target_server_id,
            'installed_at': datetime.utcnow().isoformat(),
        }
        TemplateService.save_config(config)

        result = {
            'success': True,
            'app_id': app.id,
            'app_name': app.name,
            'app_path': app_path,
            'server_id': job.target_server_id,
            'port': app_port,
            'port_accessible': port_accessible,
        }

        # Optional auto-domain: when the template opts in (top-level `auto_domain: true`)
        # and a managed-sites base domain is configured, publish the app at
        # <slug>.<base_domain> with an nginx vhost. HTTPS is applied ONLY if the
        # base domain's wildcard cert is already set up (HTTP otherwise) — this never
        # forces SSL. Best-effort and non-fatal; remote-server installs are skipped
        # (the panel's nginx can't proxy a container on another host). See
        # SiteDomainService.give_subdomain.
        auto_domain = bool(plan.get('auto_domain'))
        if not auto_domain:
            try:
                tmpl = TemplateService.get_template(plan.get('template_id'))
                auto_domain = bool(tmpl.get('success') and tmpl['template'].get('auto_domain'))
            except Exception:
                auto_domain = False
        if auto_domain and not job.target_server_id and app.app_type == 'docker' and app.port:
            try:
                from app.services.site_domain_service import SiteDomainService
                dom = SiteDomainService.give_subdomain(app)
                result['auto_domain'] = dom
                if dom.get('success'):
                    DeploymentPlanRunner(job).log(
                        'info', f"Published at {dom.get('url')}", dom)
                else:
                    DeploymentPlanRunner(job).log(
                        'warn', f"Auto-domain skipped: {dom.get('error')}", dom)
            except Exception as exc:
                result['auto_domain'] = {'success': False, 'error': str(exc)}
                DeploymentPlanRunner(job).log('warn', f"Auto-domain failed: {exc}")

        job.app_id = app.id
        job.set_result({**job.get_result(), **result})
        db.session.commit()

        DeploymentPlanRunner(job).log('info', f'Application record created: {app.name}', result)

        return {'success': True, 'job': job.to_dict(include_logs=True), **result}

    # ------------------------------------------------------------------
    # Unified job system integration (kind: deploy.install)
    # ------------------------------------------------------------------
    @classmethod
    def _enqueue_install(cls, job: DeploymentJob):
        """Hand the deployment to the unified job system for async execution.

        Replaces the former one-off daemon thread: the work now persists on the
        Queue Bus and is run by the single JobConsumer, so it survives a restart
        and is observable via /api/v1/jobs. Installs create containers/files/an
        app row and are NOT idempotent, so max_attempts=1 (no auto-retry).
        """
        from app.jobs.service import JobService
        return JobService.enqueue(
            JOB_KIND,
            payload={'deployment_job_id': job.id},
            max_attempts=1,
            owner_type='deployment_job',
            owner_id=job.id,
            correlation_id=job.correlation_id,
        )

    @staticmethod
    def _run_install_job(unified_job):
        """Unified-job handler for ``deploy.install``. Drives run_job and surfaces
        a failed deployment as a raised error so the unified job is marked failed
        too (the DeploymentJob row already carries the detailed status/logs)."""
        deployment_job_id = (unified_job.get_payload() or {}).get('deployment_job_id')
        if not deployment_job_id:
            raise ValueError('deploy.install job missing deployment_job_id')
        result = DeploymentJobService.run_job(deployment_job_id)
        if not result.get('success'):
            raise RuntimeError(result.get('error') or 'Deployment failed')
        return {
            'deployment_job_id': deployment_job_id,
            'app_id': result.get('app_id'),
            'app_name': result.get('app_name'),
        }

    @classmethod
    def register_jobs(cls):
        """Register deployment handlers with the unified job registry. Called once
        at app startup (see app/__init__.py)."""
        from app.jobs import registry
        registry.register(JOB_KIND, cls._run_install_job, replace=True)

    @staticmethod
    def _normalize_server_id(server_id: Optional[str]) -> Optional[str]:
        if not server_id or server_id == 'local':
            return None
        return server_id
