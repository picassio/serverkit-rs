"""Proving tests for PR Preview Environments.

Two testable cores are pure (no DB): domain templating and reconciliation.
The orchestration parts (settings upsert, create/destroy row writes, stale
expiry) use the ``app`` fixture for a clean DB and call service methods
directly — the job system is a no-op under ENV=testing.
"""
from datetime import datetime, timedelta

# Import the model at module load so its tables are registered on db.Model's
# metadata before the `app` fixture runs db.create_all(). In production the host
# imports it from models/__init__.py; here we import it directly.
from app.models.application_preview import (  # noqa: F401
    ApplicationPreview, ApplicationPreviewSettings)
from app.services.preview_service import PreviewService


# ── render_domain (pure) ─────────────────────────────────────────────────────

def test_render_domain_pr_number_and_app_domain():
    out = PreviewService.render_domain(
        'pr-{pr_number}.{app_domain}',
        {'pr_number': 42, 'app_domain': 'acme.com'})
    assert out == 'pr-42.acme.com'


def test_render_domain_slugifies_branch():
    out = PreviewService.render_domain(
        '{branch}.{base_domain}',
        {'branch': 'feature/Login-Page', 'base_domain': 'Acme.COM'})
    # branch slugified, whole host lowercased
    assert out == 'feature-login-page.acme.com'


def test_render_domain_base_domain_alias():
    # {app_domain} falls back to base_domain when app_domain is absent.
    out = PreviewService.render_domain(
        'pr-{pr_number}.{app_domain}',
        {'pr_number': 7, 'base_domain': 'example.org'})
    assert out == 'pr-7.example.org'


def test_render_domain_empty_template_and_unknown_placeholder():
    assert PreviewService.render_domain('', {'pr_number': 1}) == ''
    # Unknown placeholders are left intact rather than raising.
    out = PreviewService.render_domain('{nope}-{pr_number}', {'pr_number': 3})
    assert out == '{nope}-3'


# ── reconcile (pure) ─────────────────────────────────────────────────────────

def test_reconcile_creates_for_open_pr_without_preview():
    plan = PreviewService.reconcile(
        open_prs=[{'pr_number': 1, 'commit_sha': 'aaa'}],
        active_previews=[])
    assert [p['pr_number'] for p in plan['to_create']] == [1]
    assert plan['to_update'] == [] and plan['to_destroy'] == []


def test_reconcile_destroys_preview_for_closed_pr():
    preview = {'pr_number': 9, 'commit_sha': 'aaa', 'status': 'running'}
    plan = PreviewService.reconcile(open_prs=[], active_previews=[preview])
    assert plan['to_destroy'] == [preview]
    assert plan['to_create'] == [] and plan['to_update'] == []


def test_reconcile_updates_on_new_commit():
    plan = PreviewService.reconcile(
        open_prs=[{'pr_number': 5, 'commit_sha': 'newsha'}],
        active_previews=[{'pr_number': 5, 'commit_sha': 'oldsha', 'status': 'running'}])
    assert [p['pr_number'] for p in plan['to_update']] == [5]
    assert plan['to_create'] == [] and plan['to_destroy'] == []


def test_reconcile_noop_when_commit_unchanged():
    plan = PreviewService.reconcile(
        open_prs=[{'pr_number': 5, 'commit_sha': 'same'}],
        active_previews=[{'pr_number': 5, 'commit_sha': 'same', 'status': 'running'}])
    assert plan['to_create'] == [] and plan['to_update'] == [] and plan['to_destroy'] == []


def test_reconcile_ignores_already_destroyed_previews():
    # A destroyed preview is not "live": its closed PR should NOT be re-destroyed.
    plan = PreviewService.reconcile(
        open_prs=[],
        active_previews=[{'pr_number': 2, 'commit_sha': 'x', 'status': 'destroyed'}])
    assert plan['to_destroy'] == []


def test_reconcile_mixed_matrix():
    open_prs = [
        {'pr_number': 1, 'commit_sha': 'a'},   # new -> create
        {'pr_number': 2, 'commit_sha': 'b2'},  # moved -> update
        {'pr_number': 3, 'commit_sha': 'c'},   # unchanged -> noop
    ]
    active = [
        {'pr_number': 2, 'commit_sha': 'b1', 'status': 'running'},
        {'pr_number': 3, 'commit_sha': 'c', 'status': 'running'},
        {'pr_number': 4, 'commit_sha': 'd', 'status': 'running'},  # closed -> destroy
    ]
    plan = PreviewService.reconcile(open_prs, active)
    assert [p['pr_number'] for p in plan['to_create']] == [1]
    assert [p['pr_number'] for p in plan['to_update']] == [2]
    assert [p['pr_number'] for p in plan['to_destroy']] == [4]


# ── DB-backed orchestration ──────────────────────────────────────────────────

def _mk_app(name='preview-app', app_type='docker', port=8400):
    from app import db
    from app.models import User, Application
    from werkzeug.security import generate_password_hash
    u = User(email=f'{name}@prev.local', username=f'prev-{name}',
             password_hash=generate_password_hash('x'),
             role=User.ROLE_ADMIN, is_active=True)
    db.session.add(u)
    db.session.commit()
    a = Application(name=name, app_type=app_type, user_id=u.id,
                    root_path=f'/srv/{name}', port=port)
    db.session.add(a)
    db.session.commit()
    return a


def test_enable_and_get_settings(app):
    with app.app_context():
        a = _mk_app()
        # Defaults before any save.
        s = PreviewService.get_settings(a.id)
        assert s.enabled is False
        assert s.domain_template == 'pr-{pr_number}.{app_domain}'

        result = PreviewService.enable_previews(a.id, {
            'enabled': True,
            'domain_template': 'preview-{pr_number}.{app_domain}',
            'ttl_days': 3,
        })
        assert result['enabled'] is True
        assert result['domain_template'] == 'preview-{pr_number}.{app_domain}'
        assert result['ttl_days'] == 3

        # Re-fetch is persisted.
        again = PreviewService.get_settings(a.id)
        assert again.enabled is True
        assert again.ttl_days == 3


def test_create_and_destroy_writes_rows_and_flips_status(app):
    from app.models.application_preview import ApplicationPreview
    with app.app_context():
        a = _mk_app(name='create-destroy')
        PreviewService.enable_previews(a.id, {'enabled': True})

        created = PreviewService.create_preview(a, {
            'pr_number': 12, 'branch': 'feature/x', 'commit_sha': 'deadbeef',
            'pr_title': 'Add X',
        })
        assert created.get('pr_number') == 12
        assert created.get('status') == ApplicationPreview.STATUS_RUNNING
        assert created.get('domain')  # a domain was rendered

        row = ApplicationPreview.query.filter_by(application_id=a.id, pr_number=12).first()
        assert row is not None and row.status == ApplicationPreview.STATUS_RUNNING

        destroyed = PreviewService.destroy_preview(row.id)
        assert destroyed.get('status') == ApplicationPreview.STATUS_DESTROYED
        assert row.deleted_at is not None


def test_create_preview_skips_when_disabled(app):
    with app.app_context():
        a = _mk_app(name='disabled-app')
        # No enable_previews call -> previews disabled.
        res = PreviewService.create_preview(a, {'pr_number': 1})
        assert res.get('skipped') is True
        assert res.get('reason') == 'previews_disabled'


def test_expire_stale_destroys_past_due(app):
    from app import db
    from app.models.application_preview import ApplicationPreview
    with app.app_context():
        a = _mk_app(name='stale-app')
        # Stale preview (expired yesterday).
        stale = ApplicationPreview(
            application_id=a.id, pr_number=99, status=ApplicationPreview.STATUS_RUNNING,
            expires_at=datetime.utcnow() - timedelta(days=1))
        # Fresh preview (expires tomorrow) — must survive.
        fresh = ApplicationPreview(
            application_id=a.id, pr_number=100, status=ApplicationPreview.STATUS_RUNNING,
            expires_at=datetime.utcnow() + timedelta(days=1))
        db.session.add_all([stale, fresh])
        db.session.commit()

        out = PreviewService.expire_stale()
        assert out['expired'] == 1

        db.session.refresh(stale)
        db.session.refresh(fresh)
        assert stale.status == ApplicationPreview.STATUS_DESTROYED
        assert fresh.status == ApplicationPreview.STATUS_RUNNING


def test_reconcile_against_db_rows(app):
    """reconcile accepts ORM rows (attribute access), not just dicts."""
    from app import db
    from app.models.application_preview import ApplicationPreview
    with app.app_context():
        a = _mk_app(name='orm-reconcile')
        live = ApplicationPreview(
            application_id=a.id, pr_number=7, commit_sha='old',
            status=ApplicationPreview.STATUS_RUNNING)
        db.session.add(live)
        db.session.commit()

        plan = PreviewService.reconcile(
            open_prs=[{'pr_number': 7, 'commit_sha': 'new'}],
            active_previews=[live])
        assert [p['pr_number'] for p in plan['to_update']] == [7]
