# shellcheck shell=bash
#
# scripts/lib/init.sh — sourceable init-system / service-control abstraction.
#
# ServerKit installs and updates on a grab-bag of Linux distros, and the way you
# start/stop/enable a long-running service differs by init system: systemd on
# most modern boxes, OpenRC on Alpine/Gentoo, runit on Void, and plain SysV
# init.d scripts on older or stripped-down hosts. The installer/updater must be
# able to drive whichever one is present without the call sites caring which —
# otherwise every script that touches the `serverkit` service would grow its own
# `if command -v systemctl ...` ladder. This library hides that behind one small
# verb-based API so callers say "start this service" and the right thing happens.
#
# Contract (these names/behaviors are depended on by callers and unit tests —
# do not rename them or add extra public functions):
#   init_detect                → echoes: systemd|openrc|runit|sysvinit|none
#   init_start     <service>   → start a service
#   init_stop      <service>   → stop a service
#   init_enable    <service>   → enable at boot
#   init_disable   <service>   → disable at boot
#   init_reload    [service]   → with a service: reload it, propagating the
#                                tool's exit exactly like init_start/stop;
#                                with no arg: systemd daemon-reload (a
#                                successful no-op on every other init system,
#                                where the concept does not exist)
#   init_is_active <service>   → return 0 if running, 1 otherwise (no sudo)
#
# Honors:
#   INIT_OVERRIDE=...   force the detected init system (used by unit tests)
#   INIT_DRY_RUN=1      print intended commands instead of running them
#
# Everything is best-effort: when no init system can be driven, the mutating
# helpers warn loudly and return 1 rather than aborting the caller, and runit's
# enable/disable (which is symlink-based) never aborts even if the symlink op
# fails.

# Cache of the detected init system: probing /proc and PATH once per process is
# enough. init_detect honours INIT_OVERRIDE *before* this cache, so an override
# flipped per-call in tests always takes effect.
_INIT_DETECTED=""

# Prepend sudo when we are not already root and sudo exists. Used only for the
# mutating ops; init_is_active is a read and must work unprivileged.
_init_sudo() {
    if [ "$(id -u)" -ne 0 ] && command -v sudo >/dev/null 2>&1; then
        printf 'sudo'
    fi
}

# Run a privileged service command, or just print it under dry-run. Kept tiny so
# the dry-run output is exactly the command that would otherwise execute — which
# is what the installer unit tests assert against.
_init_run() {
    # Call sites pass "$(_init_sudo)" as the first arg; when root it expands to
    # an empty string, which would otherwise show up as a stray leading space in
    # the command line. Drop it so the printed/executed command is clean either
    # way (and so callers never accidentally exec the empty-string "command").
    if [ "$#" -gt 0 ] && [ -z "$1" ]; then
        shift
    fi
    if [ "${INIT_DRY_RUN:-0}" = "1" ]; then
        printf '  [init] would run: %s\n' "$*"
        return 0
    fi
    # Discard the tool's own chatter here so callers don't redirect — that
    # redirection would also swallow the dry-run line above, which the unit
    # tests assert on.
    "$@" >/dev/null 2>&1
}

# Detect the active init system. Order matters: systemd wins if it is PID 1, then
# the less-common managers, falling back to SysV and finally "none".
init_detect() {
    if [ -n "${INIT_OVERRIDE:-}" ]; then
        printf '%s\n' "$INIT_OVERRIDE"
        return 0
    fi
    if [ -n "$_INIT_DETECTED" ]; then
        printf '%s\n' "$_INIT_DETECTED"
        return 0
    fi
    local comm=""
    if [ -d /run/systemd/system ]; then
        _INIT_DETECTED="systemd"; printf 'systemd\n'; return 0
    fi
    if [ -r /proc/1/comm ]; then
        read -r comm < /proc/1/comm 2>/dev/null || comm=""
        if [ "$comm" = "systemd" ]; then
            _INIT_DETECTED="systemd"; printf 'systemd\n'; return 0
        fi
    fi
    if command -v rc-service >/dev/null 2>&1; then
        _INIT_DETECTED="openrc"; printf 'openrc\n'; return 0
    fi
    if command -v sv >/dev/null 2>&1 && { [ -d /etc/runit ] || [ -d /run/runit ]; }; then
        _INIT_DETECTED="runit"; printf 'runit\n'; return 0
    fi
    if [ -d /etc/init.d ] || command -v service >/dev/null 2>&1; then
        _INIT_DETECTED="sysvinit"; printf 'sysvinit\n'; return 0
    fi
    _INIT_DETECTED="none"; printf 'none\n'
    return 0
}

# Print "No init system detected" and fail. Centralised so every mutating
# function reports the same actionable message.
_init_no_init() {
    local action="${1:-}" svc="${2:-}"
    printf '  [init] No init system detected — cannot %s %s; start it manually\n' \
        "$action" "$svc" >&2
    return 1
}

# A zero-arg call under a `set -u` caller must warn-and-fail, never die on an
# unbound $1 expansion. Callers get the same return 1 as any other failure.
_init_no_svc() {
    printf '  [init] %s requires a service name\n' "${1:-}" >&2
}

init_start() {
    local svc="${1:-}" init
    [ -n "$svc" ] || { _init_no_svc init_start; return 1; }
    init="$(init_detect)"
    case "$init" in
        systemd)  _init_run "$(_init_sudo)" systemctl start "$svc" ;;
        openrc)   _init_run "$(_init_sudo)" rc-service "$svc" start ;;
        runit)    _init_run "$(_init_sudo)" sv up "$svc" ;;
        sysvinit) _init_run "$(_init_sudo)" service "$svc" start ;;
        *)        _init_no_init "start" "$svc" ;;
    esac
}

init_stop() {
    local svc="${1:-}" init
    [ -n "$svc" ] || { _init_no_svc init_stop; return 1; }
    init="$(init_detect)"
    case "$init" in
        systemd)  _init_run "$(_init_sudo)" systemctl stop "$svc" ;;
        openrc)   _init_run "$(_init_sudo)" rc-service "$svc" stop ;;
        runit)    _init_run "$(_init_sudo)" sv down "$svc" ;;
        sysvinit) _init_run "$(_init_sudo)" service "$svc" stop ;;
        *)        _init_no_init "stop" "$svc" ;;
    esac
}

init_enable() {
    local svc="${1:-}" init
    [ -n "$svc" ] || { _init_no_svc init_enable; return 1; }
    init="$(init_detect)"
    case "$init" in
        systemd)
            _init_run "$(_init_sudo)" systemctl enable "$svc"
            ;;
        openrc)
            _init_run "$(_init_sudo)" rc-update add "$svc"
            ;;
        runit)
            # runit enables a service by symlinking it into the supervised dir.
            # Best-effort: never abort if the symlink op fails.
            _init_run "$(_init_sudo)" ln -s "/etc/sv/$svc" "/etc/service/$svc" || true
            ;;
        sysvinit)
            if command -v update-rc.d >/dev/null 2>&1; then
                _init_run "$(_init_sudo)" update-rc.d "$svc" enable
            elif command -v chkconfig >/dev/null 2>&1; then
                _init_run "$(_init_sudo)" chkconfig "$svc" on
            else
                _init_no_init "enable" "$svc"
            fi
            ;;
        *)
            _init_no_init "enable" "$svc"
            ;;
    esac
}

init_disable() {
    local svc="${1:-}" init
    [ -n "$svc" ] || { _init_no_svc init_disable; return 1; }
    init="$(init_detect)"
    case "$init" in
        systemd)
            _init_run "$(_init_sudo)" systemctl disable "$svc"
            ;;
        openrc)
            _init_run "$(_init_sudo)" rc-update del "$svc"
            ;;
        runit)
            # Removing the supervised symlink disables the service. Best-effort.
            _init_run "$(_init_sudo)" rm -f "/etc/service/$svc" || true
            ;;
        sysvinit)
            if command -v update-rc.d >/dev/null 2>&1; then
                _init_run "$(_init_sudo)" update-rc.d "$svc" disable
            elif command -v chkconfig >/dev/null 2>&1; then
                _init_run "$(_init_sudo)" chkconfig "$svc" off
            else
                _init_no_init "disable" "$svc"
            fi
            ;;
        *)
            _init_no_init "disable" "$svc"
            ;;
    esac
}

init_reload() {
    local svc="${1:-}" init
    init="$(init_detect)"
    # Bare `init_reload` means "systemd daemon-reload"; the concept does not
    # exist on other init systems, where it is a successful no-op.
    if [ -z "$svc" ]; then
        [ "$init" = "systemd" ] || return 0
        _init_run "$(_init_sudo)" systemctl daemon-reload
        return
    fi
    # A named reload behaves exactly like init_start/stop: it propagates the
    # tool's exit, and warns-and-fails when no init system can be driven.
    case "$init" in
        systemd)  _init_run "$(_init_sudo)" systemctl reload "$svc" ;;
        openrc)   _init_run "$(_init_sudo)" rc-service "$svc" reload ;;
        runit)    _init_run "$(_init_sudo)" sv reload "$svc" ;;
        sysvinit) _init_run "$(_init_sudo)" service "$svc" reload ;;
        *)        _init_no_init "reload" "$svc" ;;
    esac
}

# Report whether a service is currently running. Read-only: no sudo, and the
# tool's own output is discarded so callers can rely purely on the exit status.
init_is_active() {
    local svc="${1:-}" init
    [ -n "$svc" ] || return 1   # no name → not running (read-only, stay quiet)
    init="$(init_detect)"
    case "$init" in
        systemd)  systemctl is-active --quiet "$svc" ;;
        openrc)   rc-service "$svc" status >/dev/null 2>&1 ;;
        runit)    sv status "$svc" >/dev/null 2>&1 ;;
        sysvinit) service "$svc" status >/dev/null 2>&1 ;;
        *)        return 1 ;;
    esac
}
