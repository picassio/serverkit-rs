"""Move bundled Git extension contributions to canonical /git.

Revision ID: 007_canonical_git_extension_route
Revises: 006_default_full_permissions
Create Date: 2026-05-03
"""

from alembic import op
import sqlalchemy as sa
import json

revision = '007_canonical_git_extension_route'
down_revision = '006_default_full_permissions'
branch_labels = None
depends_on = None


GIT_MANIFEST = {
    'name': 'serverkit-git',
    'display_name': 'Git Server',
    'version': '1.0.0',
    'description': 'Self-hosted Git server (Gitea) exposed through the ServerKit extension system.',
    'author': 'ServerKit',
    'license': 'MIT',
    'category': 'deployment',
    'homepage': 'https://github.com/jhd3197/ServerKit',
    'permissions': ['docker', 'filesystem'],
    'contributions': {
        'nav': [
            {
                'id': 'git',
                'label': 'Git',
                'route': '/git',
                'category': 'infrastructure',
                'icon': '<circle cx="18" cy="18" r="3"/><circle cx="6" cy="6" r="3"/><path d="M6 21V9a9 9 0 0 0 9 9"/>',
            },
        ],
        'routes': [
            {'path': 'git', 'component': 'GitExtensionPage'},
            {'path': 'git/:tab', 'component': 'GitExtensionPage'},
        ],
        'page_titles': {
            '/git': 'Git Repositories',
        },
        'command_palette': [
            {
                'label': 'Git',
                'path': '/git',
                'category': 'Pages',
                'keywords': 'git repos deploy extension plugin',
            },
        ],
    },
}


def upgrade():
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    if 'installed_plugins' not in inspector.get_table_names():
        return

    cols = {c['name'] for c in inspector.get_columns('installed_plugins')}
    if not {'slug', 'manifest_json'}.issubset(cols):
        return

    values = {
        'slug': 'serverkit-git',
        'display_name': GIT_MANIFEST['display_name'],
        'description': GIT_MANIFEST['description'],
        'manifest_json': json.dumps(GIT_MANIFEST),
    }
    assignments = ['manifest_json = :manifest_json']
    if 'display_name' in cols:
        assignments.append('display_name = :display_name')
    if 'description' in cols:
        assignments.append('description = :description')

    conn.execute(
        sa.text(
            f"UPDATE installed_plugins SET {', '.join(assignments)} WHERE slug = :slug"
        ),
        values,
    )


def downgrade():
    # Keep the canonical route. Reintroducing /git-ext would recreate the
    # duplicated Git/Git (ext) UI this migration removes.
    pass
