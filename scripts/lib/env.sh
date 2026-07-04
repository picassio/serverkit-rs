# shellcheck shell=bash
#
# scripts/lib/env.sh — runtime-environment detection for install/update.
#
# The installer and updater behave differently depending on where they run:
# inside a Docker/podman/LXC container the host-level service plumbing
# (systemctl units, firewall rules, journald) is either absent or off-limits;
# under WSL there is usually no real init and /proc looks Linux-ish but a lot of
# systemd assumptions break; and on a plain bare-metal/VPS box systemd is the
# normal service manager. Rather than scatter ad-hoc `[ -f /.dockerenv ]` and
# `command -v systemctl` checks through the scripts, we centralise the three
# questions the rest of the tooling actually asks into named predicates so the
# intent is obvious at the call site (`if is_container; then ...`).
#
# Design rules:
#   * Every check is SAFE on a non-Linux dev box (Windows/macOS): no file is
#     read without a `[ -f ]`/`[ -d ]` guard first, and nothing here aborts the
#     caller under `set -e`. Each predicate is a series of `if ... return 0; fi`
#     checks ending in a single `return 1`, so a non-matching grep maps to
#     "false" instead of leaking a failure exit status.
#   * Each predicate honours a force-override env var (checked FIRST) so unit
#     tests — and operators in weird environments — can pin the answer:
#       SERVERKIT_IS_CONTAINER / SERVERKIT_IS_WSL / SERVERKIT_HAS_SYSTEMD
#     each accept "1" (force true) or "0" (force false).
#   * The marker/probe paths are likewise overridable so tests can point at
#     fixtures instead of the real /proc and /run:
#       SERVERKIT_DOCKERENV_FILE   (default /.dockerenv)
#       SERVERKIT_CONTAINERENV_FILE(default /run/.containerenv)   [podman]
#       SERVERKIT_CGROUP_FILE      (default /proc/1/cgroup)
#       SERVERKIT_OSRELEASE_FILE   (default /proc/sys/kernel/osrelease)
#
# Contract (all three are predicates: return 0/true or 1/false, print nothing):
#   is_container  → running inside a container?
#   is_wsl        → running under Windows Subsystem for Linux?
#   has_systemd   → is systemd usable as the init/service manager?

# True if running inside a container (Docker, podman, LXC, systemd-nspawn, k8s).
# Several independent signals are checked because no single one is universal:
# Docker drops /.dockerenv, podman drops /run/.containerenv, nspawn/lxc export
# `container=...`, and as a last resort the init cgroup line names the runtime.
is_container() {
    # Explicit force-override wins over any probing.
    if [ "${SERVERKIT_IS_CONTAINER:-}" = "1" ]; then return 0; fi
    if [ "${SERVERKIT_IS_CONTAINER:-}" = "0" ]; then return 1; fi

    local dockerenv containerenv cgroup
    dockerenv="${SERVERKIT_DOCKERENV_FILE:-/.dockerenv}"
    containerenv="${SERVERKIT_CONTAINERENV_FILE:-/run/.containerenv}"
    cgroup="${SERVERKIT_CGROUP_FILE:-/proc/1/cgroup}"

    # Docker writes this marker into every container's root.
    if [ -f "$dockerenv" ]; then return 0; fi
    # podman's equivalent marker.
    if [ -f "$containerenv" ]; then return 0; fi
    # systemd-nspawn and many lxc setups export this into the environment.
    if [ -n "${container:-}" ]; then return 0; fi
    # Fall back to the PID-1 cgroup line, which names the runtime in most cases.
    # `-a` treats the (possibly NUL-laden) /proc file as text so grep won't bail.
    if [ -f "$cgroup" ] && grep -qaE 'docker|lxc|containerd|kubepods|podman' "$cgroup"; then
        return 0
    fi

    return 1
}

# True under Windows Subsystem for Linux. The kernel release string is the
# reliable tell on both WSL1 and WSL2 ("...-microsoft-standard-WSL2"), so we
# match it case-insensitively rather than trusting any single distro quirk.
is_wsl() {
    if [ "${SERVERKIT_IS_WSL:-}" = "1" ]; then return 0; fi
    if [ "${SERVERKIT_IS_WSL:-}" = "0" ]; then return 1; fi

    local osrelease
    osrelease="${SERVERKIT_OSRELEASE_FILE:-/proc/sys/kernel/osrelease}"

    if [ -f "$osrelease" ] && grep -qiE 'microsoft|WSL' "$osrelease"; then
        return 0
    fi

    return 1
}

# True if systemd is usable as the init/service manager. Both conditions matter:
# `systemctl` must exist AND /run/systemd/system must be present — the latter is
# created by systemd itself when it is PID 1, so its existence implies systemd is
# actually running (not just installed). This keeps us from trying to drive
# systemctl inside a container whose init is something else.
has_systemd() {
    if [ "${SERVERKIT_HAS_SYSTEMD:-}" = "1" ]; then return 0; fi
    if [ "${SERVERKIT_HAS_SYSTEMD:-}" = "0" ]; then return 1; fi

    if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
        return 0
    fi

    return 1
}
