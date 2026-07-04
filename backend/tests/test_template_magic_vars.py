"""Proving tests for declarative-catalog magic variables and catalog validation.

Covers ``TemplateService.resolve_magic_variables`` (token detection, stable
generation, each token form, dict/list shapes, and no-op when no tokens are
present) plus ``TemplateService.validate_catalog_entry``. These are pure /
unit-testable: no Docker, no network.
"""
import base64
import re

import pytest

from app.services.template_service import TemplateService as TS


# ---------------------------------------------------------------------------
# Token detection + stable generation
# ---------------------------------------------------------------------------

def test_password_token_detected_and_generated():
    text = 'MYSQL_ROOT_PASSWORD=${SERVICE_PASSWORD_DB}'
    out, generated = TS.resolve_magic_variables(text)

    assert 'SERVICE_PASSWORD_DB' in generated
    value = generated['SERVICE_PASSWORD_DB']
    # Strong password, default length, alnum (compose/shell-safe).
    assert len(value) == TS.MAGIC_PASSWORD_LENGTH
    assert value.isalnum()
    assert out == f'MYSQL_ROOT_PASSWORD={value}'
    assert '${SERVICE_PASSWORD_DB}' not in out


def test_same_token_resolves_to_one_stable_value():
    text = 'A=${SERVICE_PASSWORD_DB} B=${SERVICE_PASSWORD_DB}'
    out, generated = TS.resolve_magic_variables(text)

    # Generated exactly once for the unique token.
    assert list(generated.keys()) == ['SERVICE_PASSWORD_DB']
    value = generated['SERVICE_PASSWORD_DB']
    assert out == f'A={value} B={value}'


def test_different_names_get_different_values():
    text = '${SERVICE_PASSWORD_DB} ${SERVICE_PASSWORD_CACHE}'
    _, generated = TS.resolve_magic_variables(text)

    assert set(generated) == {'SERVICE_PASSWORD_DB', 'SERVICE_PASSWORD_CACHE'}
    assert generated['SERVICE_PASSWORD_DB'] != generated['SERVICE_PASSWORD_CACHE']


def test_user_token_form():
    _, generated = TS.resolve_magic_variables('${SERVICE_USER_APP}')
    user = generated['SERVICE_USER_APP']
    assert user.startswith('svc_app_')
    # safe identifier characters only
    assert re.fullmatch(r'svc_app_[0-9a-f]+', user)


def test_base64_token_form():
    _, generated = TS.resolve_magic_variables('SECRET=${SERVICE_BASE64_KEY}')
    val = generated['SERVICE_BASE64_KEY']
    # Decodes cleanly as base64.
    decoded = base64.b64decode(val)
    assert len(decoded) > 0


def test_fqdn_and_url_use_context():
    text = 'host=${SERVICE_FQDN_WEB} url=${SERVICE_URL_WEB}'
    out, generated = TS.resolve_magic_variables(
        text, context={'fqdn': 'app.example.com', 'scheme': 'https'}
    )
    assert generated['SERVICE_FQDN_WEB'] == 'app.example.com'
    assert generated['SERVICE_URL_WEB'] == 'https://app.example.com'
    assert out == 'host=app.example.com url=https://app.example.com'


def test_fqdn_url_fall_back_to_placeholder_without_context():
    text = '${SERVICE_FQDN_WEB} ${SERVICE_URL_WEB}'
    out, generated = TS.resolve_magic_variables(text, context={'app_name': 'myapp'})
    # No FQDN known -> documented placeholder (app_name), http scheme default.
    assert generated['SERVICE_FQDN_WEB'] == 'myapp'
    assert generated['SERVICE_URL_WEB'] == 'http://myapp'
    assert out == 'myapp http://myapp'


# ---------------------------------------------------------------------------
# Shapes: dicts / lists (a compose section) are walked and substituted in place
# ---------------------------------------------------------------------------

def test_resolves_inside_nested_dict_and_list():
    compose = {
        'services': {
            'db': {
                'environment': [
                    'MYSQL_PASSWORD=${SERVICE_PASSWORD_DB}',
                    'MYSQL_USER=${SERVICE_USER_DB}',
                ],
            },
        },
    }
    out, generated = TS.resolve_magic_variables(compose)

    assert set(generated) == {'SERVICE_PASSWORD_DB', 'SERVICE_USER_DB'}
    env = out['services']['db']['environment']
    assert env[0] == f"MYSQL_PASSWORD={generated['SERVICE_PASSWORD_DB']}"
    assert env[1] == f"MYSQL_USER={generated['SERVICE_USER_DB']}"


# ---------------------------------------------------------------------------
# No-op behavior: templates without magic tokens are unchanged
# ---------------------------------------------------------------------------

def test_noop_when_no_magic_tokens_present():
    text = 'image: nginx\nports:\n  - "${HTTP_PORT}:80"'
    out, generated = TS.resolve_magic_variables(text)
    # The plain ${HTTP_PORT} is NOT a magic token; nothing changes.
    assert generated == {}
    assert out == text


def test_non_service_dollar_vars_untouched():
    out, generated = TS.resolve_magic_variables('${APP_NAME} ${DB_PASSWORD}')
    assert generated == {}
    assert out == '${APP_NAME} ${DB_PASSWORD}'


def test_collect_magic_variables_for_template_scans_all_sections():
    template = {
        'compose': {'services': {'app': {'environment': ['P=${SERVICE_PASSWORD_X}']}}},
        'files': [{'path': '/etc/app.conf', 'content': 'user=${SERVICE_USER_X}'}],
        'scripts': {'post_install': 'echo ${SERVICE_BASE64_X}'},
    }
    generated = TS.collect_magic_variables(template)
    assert set(generated) == {'SERVICE_PASSWORD_X', 'SERVICE_USER_X', 'SERVICE_BASE64_X'}


def test_collect_magic_variables_empty_for_plain_template():
    template = {
        'compose': {'services': {'app': {'image': 'nginx', 'ports': ['${HTTP_PORT}:80']}}},
    }
    assert TS.collect_magic_variables(template) == {}


# ---------------------------------------------------------------------------
# validate_catalog_entry
# ---------------------------------------------------------------------------

def test_validate_catalog_entry_accepts_minimal_valid():
    entry = {
        'id': 'my-service',
        'name': 'My Service',
        'version': '1.0',
        'description': 'A service',
        'compose': {'services': {'app': {'image': 'nginx'}}},
    }
    result = TS.validate_catalog_entry(entry)
    assert result['valid'] is True
    assert result['errors'] == []


def test_validate_catalog_entry_flags_missing_required_fields():
    result = TS.validate_catalog_entry({'name': 'X'})
    assert result['valid'] is False
    # missing version/description and missing compose/dockerfile
    assert any('version' in e for e in result['errors'])


def test_validate_catalog_entry_rejects_bad_id_slug():
    entry = {
        'id': 'Bad ID!',
        'name': 'X', 'version': '1', 'description': 'd',
        'compose': {'services': {'app': {'image': 'nginx'}}},
    }
    result = TS.validate_catalog_entry(entry)
    assert result['valid'] is False
    assert any("'id'" in e for e in result['errors'])


def test_validate_catalog_entry_warns_unknown_var_type():
    entry = {
        'id': 'svc', 'name': 'X', 'version': '1', 'description': 'd',
        'compose': {'services': {'app': {'image': 'nginx'}}},
        'variables': [{'name': 'FOO', 'type': 'mystery'}],
    }
    result = TS.validate_catalog_entry(entry)
    assert result['valid'] is True  # warnings are non-fatal
    assert any('mystery' in w for w in result['warnings'])


def test_validate_catalog_entry_warns_on_malformed_magic_token():
    entry = {
        'id': 'svc', 'name': 'X', 'version': '1', 'description': 'd',
        'compose': {'services': {'app': {
            'image': 'nginx',
            'environment': ['BAD=${SERVICE_BOGUS_X}'],  # unknown kind
        }}},
    }
    result = TS.validate_catalog_entry(entry)
    assert any('SERVICE_BOGUS_X' in w for w in result['warnings'])


def test_validate_catalog_entry_accepts_well_formed_magic_tokens():
    entry = {
        'id': 'svc', 'name': 'X', 'version': '1', 'description': 'd',
        'compose': {'services': {'app': {
            'image': 'nginx',
            'environment': ['PW=${SERVICE_PASSWORD_DB}'],
        }}},
    }
    result = TS.validate_catalog_entry(entry)
    assert result['valid'] is True
    assert result['warnings'] == []
