"""Pytest fixtures for backend tests (Flask app, DB, client)."""
import os
import sys

import pytest

# Ensure backend root is on path
_backend = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if _backend not in sys.path:
    sys.path.insert(0, _backend)

os.environ.setdefault('FLASK_ENV', 'testing')

# Tests use a FILE-backed SQLite, not :memory:. Flask-SQLAlchemy serves an
# in-memory SQLite from a single shared connection (StaticPool), so a test that
# drives the DB from a background thread (e.g. the agent send_command round-trip
# e2e) races on that one connection and intermittently dies with
# ObjectDeletedError / PendingRollbackError. A temp file gives each thread its
# own connection with SQLite's own file locking — safe and deterministic. Each
# test still create_all/drop_all's a clean schema (see the `app` fixture).
if 'TEST_DATABASE_URL' not in os.environ:
    import tempfile
    _test_db = os.path.join(tempfile.gettempdir(), 'serverkit_test.db').replace('\\', '/')
    try:
        os.remove(_test_db)
    except OSError:
        pass
    os.environ['TEST_DATABASE_URL'] = 'sqlite:///' + _test_db


# A file-backed SQLite fsyncs on every commit, which makes the suite ~3x
# slower than :memory:. For throwaway test data that durability is pure
# overhead, so disable it and keep the journal in memory — this recovers
# roughly in-memory speed while keeping the per-connection thread-safety the
# file gives us. SQLite-only and scoped to the test process.
#
# Guarded import: some CI jobs run only the stdlib-y system-utils tests with a
# minimal dependency set (no SQLAlchemy). conftest still has to import there,
# and those jobs run no DB-backed test, so the PRAGMA tuning is simply skipped.
try:
    import sqlite3  # noqa: E402
    from sqlalchemy import event  # noqa: E402
    from sqlalchemy.engine import Engine  # noqa: E402

    @event.listens_for(Engine, 'connect')
    def _fast_sqlite_for_tests(dbapi_connection, _record):
        if isinstance(dbapi_connection, sqlite3.Connection):
            cur = dbapi_connection.cursor()
            cur.execute('PRAGMA synchronous=OFF')
            cur.execute('PRAGMA journal_mode=MEMORY')
            cur.execute('PRAGMA temp_store=MEMORY')
            cur.close()
except ImportError:
    pass


@pytest.fixture(scope='function')
def app():
    """Create Flask app with testing config and in-memory DB."""
    from app import create_app
    from app import db as _db

    app = create_app('testing')
    with app.app_context():
        _db.create_all()
        yield app
        _db.session.remove()
        _db.drop_all()


@pytest.fixture
def client(app):
    """Flask test client."""
    return app.test_client()


@pytest.fixture
def db_session(app):
    """Database session for the current test (same as app's db)."""
    from app import db
    return db


@pytest.fixture
def auth_headers(app):
    """Create an admin user and return headers with valid JWT for API tests."""
    from app import db
    from app.models import User
    from flask_jwt_extended import create_access_token
    from werkzeug.security import generate_password_hash

    with app.app_context():
        user = User(
            email='testadmin@test.local',
            username='testadmin',
            password_hash=generate_password_hash('testpass'),
            role=User.ROLE_ADMIN,
            is_active=True,
        )
        db.session.add(user)
        db.session.commit()
        token = create_access_token(identity=user.id)

    return {'Authorization': f'Bearer {token}'}
