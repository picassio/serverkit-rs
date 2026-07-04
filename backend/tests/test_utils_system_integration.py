"""Integration smoke tests for system utilities on real Linux distros.

These run WITHOUT mocks inside actual distro containers (Ubuntu, Fedora,
Rocky Linux) to verify detection logic works against real package managers
and systemctl.  Designed for CI — skipped on Windows/macOS.
"""

import os
import platform
import subprocess
import sys

import pytest

# Skip entire module on non-Linux
pytestmark = pytest.mark.skipif(
    platform.system() != 'Linux',
    reason='Integration tests require Linux',
)

# Direct import (same technique as unit tests) to avoid Flask deps.
import importlib
import types

_backend = os.path.join(os.path.dirname(__file__), os.pardir)
_mod_path = os.path.join(_backend, 'app', 'utils', 'system.py')
_spec = importlib.util.spec_from_file_location('app.utils.system', _mod_path)
_module = importlib.util.module_from_spec(_spec)

if 'app' not in sys.modules:
    sys.modules['app'] = types.ModuleType('app')
if 'app.utils' not in sys.modules:
    _utils = types.ModuleType('app.utils')
    sys.modules['app.utils'] = _utils
    sys.modules['app'].utils = _utils
sys.modules['app.utils.system'] = _module
sys.modules['app.utils'].system = _module
_spec.loader.exec_module(_module)

PackageManager = _module.PackageManager
ServiceControl = _module.ServiceControl
is_command_available = _module.is_command_available
run_privileged = _module.run_privileged


class TestPackageManagerDetection:
    """Verify PackageManager.detect() returns the correct manager for the distro."""

    def setup_method(self):
        PackageManager.reset_cache()

    def test_detect_returns_known_manager(self):
        """On any supported Linux, detect() should find apt, dnf, or yum."""
        result = PackageManager.detect()
        assert result in ('apt', 'dnf', 'yum'), (
            f'Expected apt/dnf/yum but got {result!r} — '
            f'is this an unsupported distro?'
        )

    def test_detect_matches_distro(self):
        """The detected manager should match the actual distro family."""
        manager = PackageManager.detect()

        # Read os-release to determine distro family
        distro_id = ''
        id_like = ''
        if os.path.exists('/etc/os-release'):
            with open('/etc/os-release') as f:
                for line in f:
                    if line.startswith('ID='):
                        distro_id = line.split('=', 1)[1].strip().strip('"')
                    elif line.startswith('ID_LIKE='):
                        id_like = line.split('=', 1)[1].strip().strip('"')

        apt_distros = ('ubuntu', 'debian', 'linuxmint', 'pop')
        dnf_distros = ('fedora', 'rhel', 'centos', 'rocky', 'alma', 'ol')

        if distro_id in apt_distros or any(d in id_like for d in ('debian', 'ubuntu')):
            assert manager == 'apt', f'Debian-family distro ({distro_id}) should use apt, got {manager}'
        elif distro_id in dnf_distros or 'rhel' in id_like or 'fedora' in id_like:
            assert manager in ('dnf', 'yum'), f'RHEL-family distro ({distro_id}) should use dnf/yum, got {manager}'

    def test_detect_is_cached(self):
        """Calling detect() twice should return the same cached value."""
        first = PackageManager.detect()
        second = PackageManager.detect()
        assert first == second

    def test_is_available(self):
        """On any CI Linux container, a package manager should be available."""
        assert PackageManager.is_available() is True

    def test_matches_expected_manager_from_ci(self):
        """When CI sets EXPECTED_MANAGER, verify detect() agrees."""
        expected = os.environ.get('EXPECTED_MANAGER')
        if expected is None:
            pytest.skip('EXPECTED_MANAGER not set (not running in CI matrix)')
        detected = PackageManager.detect()
        assert detected == expected, (
            f'CI matrix expects {expected!r} but detect() returned {detected!r}'
        )


class TestIsCommandAvailable:
    """Verify is_command_available() finds real binaries."""

    def test_finds_python(self):
        assert is_command_available('python3') is True

    def test_finds_bash(self):
        assert is_command_available('bash') is True

    def test_missing_binary(self):
        assert is_command_available('this_binary_does_not_exist_xyz') is False

    @pytest.mark.skipif(
        not os.path.exists('/usr/bin/apt'),
        reason='apt not available on this distro',
    )
    def test_finds_apt(self):
        assert is_command_available('apt') is True

    @pytest.mark.skipif(
        not os.path.exists('/usr/bin/dnf'),
        reason='dnf not available on this distro',
    )
    def test_finds_dnf(self):
        assert is_command_available('dnf') is True


class TestServiceControlSmoke:
    """Smoke-test ServiceControl against real systemctl (if present)."""

    @pytest.mark.skipif(
        not os.path.exists('/usr/bin/systemctl') and not os.path.exists('/bin/systemctl'),
        reason='systemctl not available in this container',
    )
    def test_is_active_nonexistent_service(self):
        """A service that doesn't exist should not be active."""
        assert ServiceControl.is_active('this_service_does_not_exist_xyz') is False

    @pytest.mark.skipif(
        not os.path.exists('/usr/bin/systemctl') and not os.path.exists('/bin/systemctl'),
        reason='systemctl not available in this container',
    )
    def test_is_enabled_nonexistent_service(self):
        assert ServiceControl.is_enabled('this_service_does_not_exist_xyz') is False
