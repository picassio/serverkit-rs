"""Tests for the Project / Environment hierarchy.

These import the Project/Environment models at module scope so they're registered
on db.Model's metadata before the `app` fixture runs db.create_all() (the global
models/__init__ wiring is added separately; the suite must not depend on it).
"""
import pytest

# Registering the new models on the metadata so create_all() builds their tables.
from app.models.project import Project  # noqa: F401
from app.models.environment import Environment  # noqa: F401
from app.models.application import Application


@pytest.fixture
def workspace(app):
    """A workspace to scope projects under."""
    from app import db
    from app.services.workspace_service import WorkspaceService
    with app.app_context():
        ws = WorkspaceService.ensure_default_workspace()
        return ws.id


def _make_app(name, workspace_id, project_id=None, environment_id=None):
    from app import db
    from app.models.user import User
    from werkzeug.security import generate_password_hash
    # An owner user (apps require a non-null user_id).
    user = User.query.filter_by(username='appowner').first()
    if user is None:
        user = User(
            email='appowner@test.local',
            username='appowner',
            password_hash=generate_password_hash('x'),
            role='admin',
            is_active=True,
        )
        db.session.add(user)
        db.session.commit()
    a = Application(
        name=name,
        app_type='docker',
        status='stopped',
        user_id=user.id,
        workspace_id=workspace_id,
        project_id=project_id,
        environment_id=environment_id,
    )
    db.session.add(a)
    db.session.commit()
    return a


def test_create_project_auto_creates_default_env(app, workspace):
    from app.services.project_service import ProjectService
    with app.app_context():
        project = ProjectService.create_project(workspace, name='My Project')
        envs = ProjectService.list_environments(project.id)
        assert len(envs) == 1
        assert envs[0].is_default is True
        assert envs[0].slug == 'production'
        assert project.slug == 'my-project'


def test_ensure_default_is_idempotent(app, workspace):
    from app.services.project_service import ProjectService
    with app.app_context():
        p1 = ProjectService.ensure_default(workspace)
        p2 = ProjectService.ensure_default(workspace)
        assert p1.id == p2.id
        # Still exactly one Default project for the workspace.
        defaults = Project.query.filter_by(workspace_id=workspace, slug='default').all()
        assert len(defaults) == 1
        # And it carries its default environment.
        assert p1.environments.count() == 1


def test_list_and_assign_apps_to_project_and_env(app, workspace):
    from app.services.project_service import ProjectService
    with app.app_context():
        project = ProjectService.create_project(workspace, name='Assignable')
        env = ProjectService.list_environments(project.id)[0]

        a = _make_app('app-one', workspace, project_id=project.id, environment_id=env.id)

        proj_apps = ProjectService.list_project_apps(project.id)
        assert [x.id for x in proj_apps] == [a.id]

        env_apps = ProjectService.list_environment_apps(env.id)
        assert [x.id for x in env_apps] == [a.id]

        # Counts surface in to_dict.
        d = project.to_dict(include_counts=True)
        assert d['app_count'] == 1
        assert d['environment_count'] == 1


def test_delete_project_refuses_when_non_empty(app, workspace):
    from app.services.project_service import ProjectService
    with app.app_context():
        project = ProjectService.create_project(workspace, name='Busy')
        env = ProjectService.list_environments(project.id)[0]
        _make_app('busy-app', workspace, project_id=project.id, environment_id=env.id)

        result = ProjectService.delete_project(project.id)
        assert result == 'has_apps'
        # Still present.
        assert ProjectService.get_project(project.id) is not None

        # Empty project deletes cleanly.
        empty = ProjectService.create_project(workspace, name='Empty')
        assert ProjectService.delete_project(empty.id) is True
        assert ProjectService.get_project(empty.id) is None


def test_environment_create_reorder_and_default_guard(app, workspace):
    from app.services.project_service import ProjectService
    with app.app_context():
        project = ProjectService.create_project(workspace, name='Envs')
        prod = ProjectService.list_environments(project.id)[0]

        from app import db
        staging = ProjectService.create_environment(project.id, name='staging')
        dev = ProjectService.create_environment(project.id, name='development')
        db.session.commit()

        # Three environments, prod first by order.
        envs = ProjectService.list_environments(project.id)
        assert [e.slug for e in envs] == ['production', 'staging', 'development']

        # Reorder: development, production, staging.
        reordered = ProjectService.reorder_environments(
            project.id, [dev.id, prod.id, staging.id])
        assert [e.slug for e in reordered] == ['development', 'production', 'staging']

        # Deleting a non-last environment succeeds.
        assert ProjectService.delete_environment(staging.id) is True
        remaining = ProjectService.list_environments(project.id)
        assert len(remaining) == 2

        # Deleting down to the last one is refused.
        assert ProjectService.delete_environment(dev.id) is True
        assert ProjectService.delete_environment(prod.id) == 'last'
        assert len(ProjectService.list_environments(project.id)) == 1


def test_delete_default_env_promotes_another(app, workspace):
    from app.services.project_service import ProjectService
    from app import db
    with app.app_context():
        project = ProjectService.create_project(workspace, name='Promote')
        prod = ProjectService.list_environments(project.id)[0]
        staging = ProjectService.create_environment(project.id, name='staging')
        db.session.commit()

        assert prod.is_default is True
        # Delete the default -> the remaining env should be promoted to default.
        assert ProjectService.delete_environment(prod.id) is True
        remaining = ProjectService.list_environments(project.id)
        assert len(remaining) == 1
        assert remaining[0].id == staging.id
        assert remaining[0].is_default is True


def test_delete_environment_detaches_apps(app, workspace):
    from app.services.project_service import ProjectService
    from app import db
    with app.app_context():
        project = ProjectService.create_project(workspace, name='Detach')
        prod = ProjectService.list_environments(project.id)[0]
        staging = ProjectService.create_environment(project.id, name='staging')
        db.session.commit()

        a = _make_app('detach-app', workspace, project_id=project.id, environment_id=staging.id)
        assert ProjectService.delete_environment(staging.id) is True

        db.session.refresh(a)
        # App keeps its project but loses the deleted environment.
        assert a.project_id == project.id
        assert a.environment_id is None
