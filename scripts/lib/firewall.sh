# shellcheck shell=bash
#
# scripts/lib/firewall.sh — sourceable firewall abstraction.
#
# Detects the active host firewall and opens/closes the ports ServerKit needs
# (80/443, optionally 5000). Fresh Rocky/Alma/RHEL/Fedora boxes ship firewalld
# active by default, which silently blocks 80/443 — making a perfectly good
# install look "broken" because the panel is unreachable. This helper fixes
# that automatically and, crucially, is reversible: callers persist exactly
# what was opened (see scripts/lib/state.sh) so uninstall can undo only those
# rules and never touch a user's own firewall config.
#
# Contract:
#   firewall_detect                      → echoes: firewalld|ufw|nftables|iptables|none
#   firewall_open  <backend> <port...>   → open ports (each "80/tcp")
#   firewall_close <backend> <port...>   → close the same ports
#
# Honors:
#   FW_DRY_RUN=1          print intended commands instead of running them
#   FIREWALL_BACKEND=...  force the detected backend (used by unit tests)
#
# Everything is best-effort: a firewall we can't drive cleanly degrades to a
# warning rather than aborting the install.

# Run a privileged firewall command, or just print it under dry-run. Kept tiny
# so the dry-run output is exactly the command that would otherwise execute —
# which is what the installer unit tests assert against.
_fw_run() {
    if [ "${FW_DRY_RUN:-0}" = "1" ]; then
        printf '  [firewall] would run: %s\n' "$*"
        return 0
    fi
    # Discard the tool's own chatter ("success", rule-exists noise) here so
    # callers don't redirect — that redirection would also swallow the dry-run
    # line above, which the installer unit tests assert on.
    "$@" >/dev/null 2>&1
}

# Detect the active firewall. Order matters: a box can have several of these
# binaries installed but only one actually managing the input chain, so we pick
# the highest-level manager that reports itself active.
firewall_detect() {
    if [ -n "${FIREWALL_BACKEND:-}" ]; then
        printf '%s\n' "$FIREWALL_BACKEND"
        return 0
    fi
    if command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state >/dev/null 2>&1; then
        printf 'firewalld\n'; return 0
    fi
    if command -v ufw >/dev/null 2>&1; then
        # Capture-then-case rather than `ufw status | grep -q`: grep -q exits
        # at the first match, ufw can then die of SIGPIPE, and under a caller's
        # pipefail the pipeline goes non-zero — misdetecting an active ufw.
        local ufw_out
        ufw_out="$(ufw status 2>/dev/null || true)"
        case "$ufw_out" in
            *[Ss]tatus:*[Ii]nactive*) : ;;  # "inactive" contains "active" — rule it out first
            *[Ss]tatus:*[Aa]ctive*)   printf 'ufw\n'; return 0 ;;
        esac
    fi
    if command -v nft >/dev/null 2>&1 && nft list ruleset >/dev/null 2>&1; then
        printf 'nftables\n'; return 0
    fi
    if command -v iptables >/dev/null 2>&1; then
        printf 'iptables\n'; return 0
    fi
    printf 'none\n'
}

# Split "80/tcp" into port + proto (proto defaults to tcp).
_fw_port()  { printf '%s' "${1%%/*}"; }
_fw_proto() { case "$1" in */*) printf '%s' "${1##*/}" ;; *) printf 'tcp' ;; esac; }

firewall_open() {
    # No backend argument (a zero-arg call under a `set -u` caller) is a
    # no-op, not an unbound-variable abort.
    local backend="${1:-}"
    [ -n "$backend" ] || return 0
    shift
    local spec port proto
    case "$backend" in
        firewalld)
            for spec in "$@"; do
                port="$(_fw_port "$spec")"; proto="$(_fw_proto "$spec")"
                _fw_run firewall-cmd --permanent "--add-port=${port}/${proto}" || true
            done
            _fw_run firewall-cmd --reload || true
            ;;
        ufw)
            for spec in "$@"; do
                port="$(_fw_port "$spec")"; proto="$(_fw_proto "$spec")"
                _fw_run ufw allow "${port}/${proto}" || true
            done
            ;;
        iptables)
            for spec in "$@"; do
                port="$(_fw_port "$spec")"; proto="$(_fw_proto "$spec")"
                # -C tests for an existing rule so we stay idempotent; -I only if absent.
                _fw_run iptables -C INPUT -p "$proto" --dport "$port" -j ACCEPT || \
                    _fw_run iptables -I INPUT -p "$proto" --dport "$port" -j ACCEPT || true
            done
            ;;
        nftables)
            for spec in "$@"; do
                port="$(_fw_port "$spec")"; proto="$(_fw_proto "$spec")"
                # Best-effort: requires an inet filter/input chain to exist.
                _fw_run nft add rule inet filter input "$proto" dport "$port" accept || true
            done
            ;;
        none|*)
            return 0
            ;;
    esac
}

firewall_close() {
    # Same zero-arg guard as firewall_open.
    local backend="${1:-}"
    [ -n "$backend" ] || return 0
    shift
    local spec port proto
    case "$backend" in
        firewalld)
            for spec in "$@"; do
                port="$(_fw_port "$spec")"; proto="$(_fw_proto "$spec")"
                _fw_run firewall-cmd --permanent "--remove-port=${port}/${proto}" || true
            done
            _fw_run firewall-cmd --reload || true
            ;;
        ufw)
            for spec in "$@"; do
                port="$(_fw_port "$spec")"; proto="$(_fw_proto "$spec")"
                _fw_run ufw delete allow "${port}/${proto}" || true
            done
            ;;
        iptables)
            for spec in "$@"; do
                port="$(_fw_port "$spec")"; proto="$(_fw_proto "$spec")"
                _fw_run iptables -D INPUT -p "$proto" --dport "$port" -j ACCEPT || true
            done
            ;;
        nftables|none|*)
            # nftables rule deletion needs the rule handle; we can't reliably
            # find it after the fact, so leave it to the operator.
            return 0
            ;;
    esac
}

# Warn-and-document fallback text for when no firewall could be driven.
firewall_manual_hint() {
    local ports="${1:-}"
    printf 'Open these ports manually so the panel is reachable: %s\n' "$ports"
}
