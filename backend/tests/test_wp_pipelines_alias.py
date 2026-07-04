"""§2 unification: /api/v1/wordpress/pipelines aliases /wordpress/projects.

"WordPress Projects" was renamed to "Pipelines" to end the collision with the
generic /projects surface. The environment-pipeline blueprint is mounted under
both prefixes, so every route resolves identically during the deprecation
window.
"""


def test_pipelines_alias_matches_projects_list(client, auth_headers):
    projects_res = client.get('/api/v1/wordpress/projects', headers=auth_headers)
    pipelines_res = client.get('/api/v1/wordpress/pipelines', headers=auth_headers)

    assert projects_res.status_code == 200
    assert pipelines_res.status_code == 200
    assert pipelines_res.get_json() == projects_res.get_json()


def test_both_pipeline_prefixes_registered():
    from app import create_app

    application = create_app('testing')
    rules = {r.rule for r in application.url_map.iter_rules()}
    has_projects = any(r.startswith('/api/v1/wordpress/projects') for r in rules)
    has_pipelines = any(r.startswith('/api/v1/wordpress/pipelines') for r in rules)
    assert has_projects
    assert has_pipelines
