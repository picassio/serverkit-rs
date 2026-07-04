from app.services.repository_manifest_service import RepositoryManifestService


def test_detects_agentsite_style_manifests(tmp_path):
    (tmp_path / 'Dockerfile').write_text('FROM python:3.11\nEXPOSE 6391\n', encoding='utf-8')
    (tmp_path / 'docker-compose.yml').write_text(
        '''
services:
  agentsite:
    build: .
    ports:
      - "6391:6391"
    env_file:
      - .env
''',
        encoding='utf-8',
    )
    (tmp_path / 'render.yaml').write_text(
        '''
services:
  - type: web
    name: agentsite
    runtime: docker
    dockerfilePath: ./Dockerfile
    envVars:
      - key: AGENTSITE_PORT
        value: "6391"
      - key: OPENAI_API_KEY
        sync: false
''',
        encoding='utf-8',
    )
    (tmp_path / 'railway.json').write_text(
        '''
{
  "build": { "dockerfilePath": "Dockerfile" },
  "deploy": { "healthcheckPath": "/api/health" }
}
''',
        encoding='utf-8',
    )
    (tmp_path / 'app.json').write_text(
        '''
{
  "name": "AgentSite",
  "stack": "container",
  "env": {
    "CLAUDE_API_KEY": { "required": false }
  }
}
''',
        encoding='utf-8',
    )

    manifest = RepositoryManifestService.analyze_path(str(tmp_path))

    assert manifest['strategy'] == 'docker_compose'
    assert manifest['recommended']['app_type'] == 'docker'
    assert manifest['recommended']['build_method'] == 'dockerfile'
    assert manifest['recommended']['port'] == 6391
    assert manifest['recommended']['healthcheck_path'] == '/api/health'
    assert {item['file'] for item in manifest['manifests']} >= {
        'docker-compose.yml',
        'render.yaml',
        'railway.json',
        'Dockerfile',
        'app.json',
    }
    assert 6391 in manifest['ports']
    env = {item['key']: item for item in manifest['env']}
    assert env['OPENAI_API_KEY']['secret'] is True
    assert env['OPENAI_API_KEY']['required'] is True
    assert env['.env']['kind'] == 'env_file'


def test_serverkit_manifest_takes_priority(tmp_path):
    (tmp_path / 'Dockerfile').write_text('FROM nginx\n', encoding='utf-8')
    (tmp_path / 'serverkit.json').write_text(
        '''
{
  "app_type": "flask",
  "build": { "method": "nixpacks" },
  "deploy": { "port": 8000, "startCommand": "uvicorn app:app" }
}
''',
        encoding='utf-8',
    )

    manifest = RepositoryManifestService.analyze_path(str(tmp_path))

    assert manifest['strategy'] == 'serverkit'
    assert manifest['recommended']['app_type'] == 'flask'
    assert manifest['recommended']['build_method'] == 'nixpacks'
    assert manifest['recommended']['port'] == 8000
    assert manifest['recommended']['custom_start_cmd'] == 'uvicorn app:app'
