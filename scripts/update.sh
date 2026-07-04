#!/bin/bash
#
# ServerKit updater — atomic blue/green, pre-flight checked, offline-capable.
#
# Usage:
#   bash /opt/serverkit/scripts/update.sh
#   bash /opt/serverkit/scripts/update.sh --dry-run
#   bash /opt/serverkit/scripts/update.sh --branch dev
#   bash /opt/serverkit/scripts/update.sh --release [v1.7.0]
#   SERVERKIT_OFFLINE_TARBALL=/tmp/serverkit-v1.7.0-linux-amd64.tar.gz bash /opt/serverkit/scripts/update.sh
#
# -E (errtrace) makes the ERR trap fire for failures *inside* functions/subshells
# too — without it a silent death deep in a helper would never be reported.
set -Eeuo pipefail

# ---------------------------------------------------------------------------
# Configuration + argument parsing
# ---------------------------------------------------------------------------
DRY_RUN=0
FORCE_UPDATE=0
TARGET_BRANCH=""
USE_RELEASE="${INSTALL_FROM_RELEASE:-0}"
RELEASE_VERSION="${SERVERKIT_VERSION:-}"

# Captured before parsing so the self-update re-exec can forward them verbatim.
ORIG_ARGS=("$@")

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run|-n)
            DRY_RUN=1
            shift
            ;;
        --force|-f)
            FORCE_UPDATE=1
            shift
            ;;
        --branch|-b)
            TARGET_BRANCH="$2"
            shift 2
            ;;
        --release|-r)
            USE_RELEASE=1
            if [[ -n "${2:-}" ]] && [[ ! "$2" =~ ^- ]]; then
                RELEASE_VERSION="$2"
                shift 2
            else
                shift
            fi
            ;;
        --help|-h)
            cat <<'EOF'
Usage: update.sh [OPTIONS]

Options:
  --dry-run, -n           Show what would happen without making changes
  --force, -f             Skip version comparison and update anyway
  --branch <name>, -b     Update from a git branch instead of main
  --release [version], -r Update from a release tarball
  --help, -h              Show this help message

Environment:
  SERVERKIT_DIR           Active install directory (default: /opt/serverkit)
  SERVERKIT_VENV_DIR      Python venv path (default: $SERVERKIT_DIR/venv)
  SERVERKIT_OFFLINE_TARBALL  Local release tarball to use instead of downloading
  SERVERKIT_MIRROR_URL    Base URL for release tarballs/checksums
  GITHUB_REPO             GitHub org/repo for source and releases
EOF
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
SERVERKIT_DIR="${SERVERKIT_DIR:-/opt/serverkit}"
INSTALL_DIR="$SERVERKIT_DIR"
BASE_NAME="$(basename "$INSTALL_DIR")"
BASE_DIR="$(dirname "$INSTALL_DIR")"
DIR_A="$BASE_DIR/${BASE_NAME}-a"
DIR_B="$BASE_DIR/${BASE_NAME}-b"
VENV_DIR="${SERVERKIT_VENV_DIR:-$INSTALL_DIR/venv}"
BACKUP_DIR="/var/backups/serverkit"
LOG_DIR="/var/log/serverkit"
CONFIG_DIR="${SERVERKIT_CONFIG_DIR:-/etc/serverkit}"
LOCK_FILE="${SERVERKIT_LOCK_FILE:-/var/lock/serverkit-update.lock}"

# System integration dirs — overridable so the config-refresh logic can be
# exercised against fixtures in tests instead of the host's real /etc.
NGINX_DIR="${SERVERKIT_NGINX_DIR:-/etc/nginx}"
LETSENCRYPT_DIR="${SERVERKIT_LETSENCRYPT_DIR:-/etc/letsencrypt}"
SYSTEMD_DIR="${SERVERKIT_SYSTEMD_DIR:-/etc/systemd/system}"
# Per-app nginx location snippets — one proxy_pass to each managed app's
# container. The updater probes these to prove apps stayed up across the switch.
APP_LOCATIONS_DIR="${SERVERKIT_APP_LOCATIONS_DIR:-/etc/nginx/serverkit-locations}"

GITHUB_REPO="${GITHUB_REPO:-jhd3197/ServerKit}"
SERVERKIT_OFFLINE_TARBALL="${SERVERKIT_OFFLINE_TARBALL:-}"
SERVERKIT_MIRROR_URL="${SERVERKIT_MIRROR_URL:-}"
BACKEND_SERVICE="serverkit"

# ---------------------------------------------------------------------------
# Terminal styling
# ---------------------------------------------------------------------------
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ] && [ "${TERM:-dumb}" != "dumb" ]; then
    ESC=$'\033'
    RST="${ESC}[0m"; BLD="${ESC}[1m"
    paint() { printf '%s[38;2;%d;%d;%dm' "$ESC" "$1" "$2" "$3"; }
else
    RST=''; BLD=''
    paint() { :; }
fi

V3="$(paint 139 92 246)"; V4="$(paint 124 58 237)"
PAPER="$(paint 237 233 254)"; FOG="$(paint 113 108 140)"
HUE_OK="$(paint 52 211 153)"; HUE_WARN="$(paint 250 204 21)"
HUE_ERR="$(paint 248 113 113)"; HUE_LINK="$(paint 103 232 249)"

good()  { printf '  %s✔%s %s\n' "$HUE_OK"   "$RST" "$1"; }
warn()  { printf '  %s▴%s %s\n' "$HUE_WARN" "$RST" "$1"; }
# Every hard failure points at the full log so a stuck box is debuggable from a
# single line (UPDATE_LOG is empty until init_logging runs, hence the guard).
halt()  {
    printf '  %s✘%s %s\n' "$HUE_ERR"  "$RST" "$1" >&2
    [ -n "${UPDATE_LOG:-}" ] && printf '  %sfull log: %s%s\n' "$FOG" "$UPDATE_LOG" "$RST" >&2
    exit 1
}
step()  { printf '  %s❯%s %s\n' "$HUE_LINK" "$RST" "$1"; }
info()  { printf '  %s•%s %s\n' "$FOG"      "$RST" "$1"; }

STARTED_AT=0
PHASE_N=0
clock() {
    [ "$STARTED_AT" -gt 0 ] || { printf ''; return; }
    local secs=$(( $(date +%s) - STARTED_AT ))
    printf '%dm %02ds' "$((secs / 60))" "$((secs % 60))"
}
LAST_PHASE="startup"
phase() {
    PHASE_N=$((PHASE_N + 1))
    LAST_PHASE="$1"
    printf '\n  %s%s%02d%s  %s%s%s  %s%s%s\n' \
        "$BLD" "$V3" "$PHASE_N" "$RST" "$BLD" "$1" "$RST" "$FOG" "$(clock)" "$RST"
    printf '  %s%s%s\n\n' "$V4" "──────────────────────────────────────" "$RST"
}

# ---------------------------------------------------------------------------
# L5 — Loud failures + always-on logging
# ---------------------------------------------------------------------------
# `set -euo pipefail` makes any unguarded command abort the script. Without this
# layer that abort is *silent* — you just land back at the prompt with no idea
# which phase, line, or command died (exactly the failure mode that made this
# updater so hard to debug). init_logging mirrors everything to a timestamped
# log; the ERR trap turns every abort into a labelled, actionable message.
UPDATE_LOG=""
init_logging() {
    [ -n "${SERVERKIT_NO_LOG:-}" ] && return 0
    mkdir -p "$LOG_DIR" 2>/dev/null || true
    if [ -d "$LOG_DIR" ] && [ -w "$LOG_DIR" ]; then
        UPDATE_LOG="$LOG_DIR/update-$(date +%Y%m%d-%H%M%S).log"
        # Keep output on the terminal *and* append it to the log. Colors were
        # already resolved above from the real TTY, so they survive the pipe.
        exec > >(tee -a "$UPDATE_LOG") 2>&1
        info "Logging to $UPDATE_LOG"
    fi
}

report_failure() {
    local rc=$1 line=$2 cmd=$3
    printf '\n  %s✘  Update aborted%s during %s%s%s\n' \
        "$HUE_ERR" "$RST" "$BLD" "$LAST_PHASE" "$RST" >&2
    printf '     %sexit %s · line %s · %s%s\n' "$FOG" "$rc" "$line" "$cmd" "$RST" >&2
    # `if`, not a trailing `&&` list: with UPDATE_LOG empty (fresh box before
    # init_logging, SERVERKIT_NO_LOG=1) the list form returns 1 from the
    # reporter itself — the same species as the July 2 outage.
    if [ -n "$UPDATE_LOG" ]; then
        printf '     %sfull log: %s%s\n' "$FOG" "$UPDATE_LOG" "$RST" >&2
    fi
}

# ---------------------------------------------------------------------------
# Self-updating bootstrap + run lock
# ---------------------------------------------------------------------------
# The updater that runs is whatever is installed on the box — so a *stale*
# update.sh (one predating a bug fix or a new deployment shape) fails in ways
# the current code already handles, and it can't fix itself. This is the exact
# trap that left a box stuck: an old updater died before it could install the
# fixed one. Before doing any work, fetch the newest update.sh for the target
# ref and re-exec into it. From this version on, "just run serverkit update"
# is reliable no matter how old the box is.
#
# CHANNEL DECISION (2026-07, scripts-reliability round 2): stable installs keep
# fetching this file from *main* — no release buffer. The flip side is that any
# updater bug merged to main is live on every box instantly, so merges touching
# scripts/** are gated by the real install+update e2e job in scripts-ci.yml
# (update-e2e). Fetching from the latest release tag was considered and
# rejected: a broken *released* updater would need a whole new release to fix,
# while main + a CI gate keeps the one-push-fixes-the-fleet property that
# resolved the 2026-07-02 incident.
SELF_PATH="${BASH_SOURCE[0]}"
maybe_reexec_latest_updater() {
    [ -n "${SERVERKIT_UPDATER_REEXECED:-}" ] && return 0    # already the latest
    [ "${SERVERKIT_NO_SELF_UPDATE:-0}" = "1" ] && return 0  # opt-out / tests
    [ "$DRY_RUN" = "1" ] && return 0
    [ -n "$SERVERKIT_OFFLINE_TARBALL" ] && return 0         # offline: nothing to fetch
    command -v curl &>/dev/null || return 0

    local ref="main"
    [ -n "$TARGET_BRANCH" ] && ref="$TARGET_BRANCH"
    [ "$USE_RELEASE" = "1" ] && [ -n "$RELEASE_VERSION" ] && ref="$RELEASE_VERSION"

    local url tmp
    url="https://raw.githubusercontent.com/${GITHUB_REPO}/${ref}/scripts/update.sh"
    tmp="$(mktemp)"
    if ! curl -fsSL --max-time 20 "$url" -o "$tmp" 2>/dev/null || [ ! -s "$tmp" ]; then
        rm -f "$tmp"; return 0                               # fetch failed → use local
    fi
    # Swap only if it genuinely differs AND is valid bash (never re-exec into a
    # truncated/corrupt download).
    if ! cmp -s "$tmp" "$SELF_PATH" && bash -n "$tmp" 2>/dev/null; then
        info "Refreshing the updater itself from ${ref}..."
        chmod +x "$tmp" 2>/dev/null || true
        # ${arr[@]+...}: expanding an EMPTY array as "${arr[@]}" is an
        # unbound-variable error under set -u on bash < 4.4 (older distros).
        SERVERKIT_UPDATER_REEXECED=1 exec bash "$tmp" ${ORIG_ARGS[@]+"${ORIG_ARGS[@]}"}
    fi
    rm -f "$tmp"
}

# Stop two concurrent updates clobbering each other (e.g. both cloning into the
# same blue/green slot, or racing docker compose). The lock auto-releases when
# the script exits, since the fd closes.
acquire_update_lock() {
    [ "$DRY_RUN" = "1" ] && return 0
    command -v flock &>/dev/null || return 0
    mkdir -p "$(dirname "$LOCK_FILE")" 2>/dev/null || true
    # Brace-group so a failed open (unwritable path) is caught here instead of
    # the redirection error aborting the whole script — then just skip locking.
    { exec 9>"$LOCK_FILE"; } 2>/dev/null || return 0
    if ! flock -n 9; then
        halt "Another 'serverkit update' is already running (lock: $LOCK_FILE)."
    fi
}

# ---------------------------------------------------------------------------
# Utility helpers
# ---------------------------------------------------------------------------
run_or_dry() {
    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would run: $*"
    else
        "$@"
    fi
}

# Like run_or_dry, but runs the command inside <dir>. Under --dry-run it reports
# the working directory it *would* run in instead of silently dropping the cd —
# a chained `run_or_dry cd X && run_or_dry docker compose ...` used to print the
# compose command with no hint of where it would execute.
run_in_dir() {
    local dir="$1"; shift
    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] (cwd: $dir) would run: $*"
    else
        ( cd "$dir" && "$@" )
    fi
}

# ---------------------------------------------------------------------------
# Version comparison — skip the whole update when already current.
# ---------------------------------------------------------------------------
local_version() {
    tr -d '\n\r ' < "$INSTALL_DIR/VERSION" 2>/dev/null || true
}

# True when two version strings match ignoring a leading "v" (1.7.1 == v1.7.1).
versions_equal() {
    [ "${1#v}" = "${2#v}" ]
}

# Resolve the release tag we would update to (pinned, or the latest on GitHub).
remote_release_tag() {
    if [ -n "$RELEASE_VERSION" ]; then
        printf '%s' "$RELEASE_VERSION"
        return 0
    fi
    curl -sf "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" 2>/dev/null \
        | grep '"tag_name"' | head -1 | cut -d'"' -f4 || true
}

# Return 0 when the install is already at the target version/ref (the caller
# should then skip). Best-effort: --force always proceeds; offline always
# proceeds (can't compare); an indeterminate comparison proceeds with the
# update rather than wrongly blocking it.
is_already_current() {
    [ "$FORCE_UPDATE" = "1" ] && return 1
    [ -n "$SERVERKIT_OFFLINE_TARBALL" ] && return 1

    if [ "$USE_RELEASE" = "1" ]; then
        local target local_v
        target="$(remote_release_tag)"
        [ -n "$target" ] || return 1
        local_v="$(local_version)"
        [ -n "$local_v" ] || return 1
        versions_equal "$local_v" "$target" && return 0
        return 1
    fi

    # Branch / main mode: compare the local checkout's HEAD against the remote
    # branch HEAD. ls-remote needs no fetch and works on shallow clones.
    local branch local_head remote_head
    branch="${TARGET_BRANCH:-main}"
    git -C "$INSTALL_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1 || return 1
    local_head="$(git -C "$INSTALL_DIR" rev-parse HEAD 2>/dev/null || true)"
    remote_head="$(git -C "$INSTALL_DIR" ls-remote "https://github.com/${GITHUB_REPO}.git" \
        "refs/heads/$branch" 2>/dev/null | awk '{print $1}' | head -1)"
    [ -n "$local_head" ] && [ -n "$remote_head" ] && [ "$local_head" = "$remote_head" ] && return 0
    return 1
}

# If already current, announce it and exit cleanly (unless --force).
version_gate() {
    if is_already_current; then
        local v; v="$(local_version)"
        good "Already up to date (version ${v:-unknown}). Use --force to update anyway."
        exit 0
    fi
}

wait_for_service() {
    local unit="$1" target_state="$2" timeout="${3:-30}"
    local waited=0
    while [ "$waited" -lt "$timeout" ]; do
        if systemctl is-active --quiet "$unit" 2>/dev/null; then
            [ "$target_state" = "active" ] && return 0
        else
            [ "$target_state" = "inactive" ] && return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done
    return 1
}

# Apply the (possibly refreshed) nginx config WITHOUT dropping traffic.
#
# Host nginx is the front door for every managed app — it reverse-proxies their
# containers via /etc/nginx/serverkit-locations/*.conf. Stopping it (the old
# behaviour) blacked out every hosted site for the whole switch window, so a
# panel update became an outage for unrelated apps. `nginx -s reload` keeps the
# listening sockets and in-flight connections alive while swapping config, so
# apps never blink. The updater only ever changes the *config* (refresh_config),
# never anything nginx serves from the blue/green slot, so a reload is all that
# is ever needed. If nginx happens to be down, start it instead.
reload_nginx_graceful() {
    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would reload nginx (graceful, zero-downtime)"
        return 0
    fi
    if systemctl is-active --quiet nginx 2>/dev/null; then
        # Validate before reloading: a bad config would otherwise leave nginx
        # running the OLD config (reload is rejected) — non-fatal, but warn loudly.
        if nginx -t >/dev/null 2>&1; then
            systemctl reload nginx 2>/dev/null \
                || run_or_dry systemctl reload nginx \
                || warn "nginx reload failed — apps keep serving the previous config"
        else
            warn "nginx config test failed — skipping reload (apps keep serving the previous config)"
        fi
    else
        warn "nginx was not running — starting it"
        # Never propagate a failed start: nginx being down is a pre-existing
        # condition, and aborting here (post-switch) would fire a rollback that
        # cannot fix nginx either — warn and let the operator handle it.
        run_or_dry systemctl start nginx \
            || warn "nginx failed to start — check 'systemctl status nginx'"
        wait_for_service nginx active 15 || warn "nginx did not report active within 15 seconds"
    fi
}

# Label a slot's SPA bundle for SELinux-enforcing hosts so nginx can read the
# static panel assets it now serves (see install.sh selinux_label_frontend_dist).
# Best-effort and a no-op on permissive/disabled boxes or where the tools are
# absent. Takes the slot's real path (blue/green resolves through a symlink).
selinux_label_dist() {
    local dir="$1"
    [ "$DRY_RUN" = "1" ] && return 0
    command -v selinuxenabled &>/dev/null && selinuxenabled 2>/dev/null || return 0
    local dist="${dir}/frontend/dist"
    [ -d "$dist" ] || return 0
    if command -v semanage &>/dev/null && command -v restorecon &>/dev/null; then
        semanage fcontext -a -t httpd_sys_content_t "${dist}(/.*)?" 2>/dev/null \
            || semanage fcontext -m -t httpd_sys_content_t "${dist}(/.*)?" 2>/dev/null || true
        restorecon -R "$dist" 2>/dev/null || true
    elif command -v chcon &>/dev/null; then
        chcon -R -t httpd_sys_content_t "$dist" 2>/dev/null || true
    fi
}

# ---------------------------------------------------------------------------
# Managed-app uptime verification
# ---------------------------------------------------------------------------
# The whole point of the reload-not-stop discipline is that hosted apps never go
# down when the panel updates. These helpers turn that promise into something the
# updater actually checks: snapshot which app upstreams are reachable before the
# switch, re-probe after the reload, and shout if any app that WAS serving has
# stopped answering.

# Discover the upstreams (host:port) host nginx reverse-proxies for managed apps
# by scanning the per-app location snippets. One unique "host:port" per line.
discover_app_upstreams() {
    [ -d "$APP_LOCATIONS_DIR" ] || return 0
    grep -rhoE 'proxy_pass[[:space:]]+https?://127\.0\.0\.1:[0-9]+' "$APP_LOCATIONS_DIR" 2>/dev/null \
        | grep -oE '127\.0\.0\.1:[0-9]+' | sort -u || true
}

# Probe each upstream on stdin; emit "host:port up" / "host:port down". "up" means
# the app container answered at all (ANY HTTP status — a 4xx/5xx app is still
# reachable; a refused/timed-out connection is not), which is exactly what nginx
# needs to keep fronting it.
probe_app_upstreams() {
    local up
    while IFS= read -r up; do
        [ -n "$up" ] || continue
        if curl -s -o /dev/null --max-time 4 "http://$up/" 2>/dev/null; then
            printf '%s up\n' "$up"
        else
            printf '%s down\n' "$up"
        fi
    done
}

# A reachability snapshot for every fronted app: "host:port state" lines.
snapshot_app_reachability() {
    [ "$DRY_RUN" = "1" ] && return 0
    discover_app_upstreams | probe_app_upstreams
}

# Compare a pre-update snapshot with a post-update one. Warn for any app that was
# reachable before and is not now (the update disrupted a workload). Non-zero if
# any regression is found; an app that was already down before the update is not
# counted against us.
report_app_uptime_regressions() {
    local before="$1" after="$2" regressed=0 total=0 line up state
    while IFS= read -r line; do
        [ -n "$line" ] || continue
        up="${line% *}"; state="${line##* }"
        total=$((total + 1))
        [ "$state" = "up" ] || continue
        if printf '%s\n' "$after" | grep -qx "$up down"; then
            warn "App upstream $up was reachable before the update but is DOWN now"
            regressed=$((regressed + 1))
        fi
    done <<EOF
$before
EOF
    if [ "$total" -eq 0 ]; then
        info "No managed apps fronted by nginx — nothing to verify"
    elif [ "$regressed" -eq 0 ]; then
        good "All $total managed app(s) stayed reachable across the update"
    else
        warn "$regressed of $total managed app(s) went down across the update"
    fi
    [ "$regressed" -eq 0 ]
}

# Resolve the currently active real directory behind the symlink.
active_real_dir() {
    if [ -L "$INSTALL_DIR" ]; then
        readlink -f "$INSTALL_DIR"
    elif [ -d "$INSTALL_DIR" ]; then
        echo "$INSTALL_DIR"
    else
        echo ""
    fi
}

# Return the inactive blue/green directory.
next_real_dir() {
    local active
    active="$(active_real_dir)"
    if [ "$active" = "$DIR_A" ]; then
        echo "$DIR_B"
    else
        echo "$DIR_A"
    fi
}

# ---------------------------------------------------------------------------
# Pre-flight checks
# ---------------------------------------------------------------------------
preflight_check() {
    phase "Pre-flight Checks"

    # Root
    if [ "$EUID" -ne 0 ]; then
        halt "Run as root."
    fi
    good "Running as root"

    # Install directory exists or can be created
    if [ ! -e "$BASE_DIR" ]; then
        halt "Base directory $BASE_DIR does not exist."
    fi
    good "Install base directory exists"

    # Python version
    local py_bin py_version
    py_bin="$(locate_python 2>/dev/null || true)"
    if [ -z "$py_bin" ]; then
        halt "Python 3.11 or 3.12 is required. Install python3.11/3.12 and python3-venv."
    fi
    py_version="$($py_bin -c 'import sys; print(".".join(map(str, sys.version_info[:2])))')"
    good "Python $py_version available ($py_bin)"

    # Required commands. Source-mode updates build the SPA on the host, so npm
    # must be checked up front — discovering it missing mid-update would abort
    # after the new slot is already half-built. Release tarballs ship a
    # prebuilt dist and stay npm-free.
    local cmd missing=() required=(git curl tar rsync systemctl nginx docker python3)
    [ "$USE_RELEASE" = "1" ] || required+=(npm)
    for cmd in "${required[@]}"; do
        command -v "$cmd" &>/dev/null || missing+=("$cmd")
    done
    if ! docker compose version &>/dev/null && ! docker-compose --version &>/dev/null; then
        missing+=("docker compose")
    fi
    if [ ${#missing[@]} -gt 0 ]; then
        halt "Missing required tools: ${missing[*]}"
    fi
    good "Required tools available"

    # Disk space (need 2 GiB free on the install filesystem)
    local avail_kb avail_gb
    # -P (POSIX format) pins each filesystem to one line — a long device name
    # otherwise wraps and shifts the column the awk parse reads.
    avail_kb="$(df -Pk "$BASE_DIR" | awk 'NR==2 {print $4}')"
    avail_gb=$((avail_kb / 1024 / 1024))
    if [ "$avail_gb" -lt 2 ]; then
        halt "Insufficient disk space on $BASE_DIR: ${avail_gb} GiB free (need at least 2 GiB)."
    fi
    good "Disk space OK (${avail_gb} GiB free)"

    # Memory (need 512 MiB free)
    local mem_avail_mb
    mem_avail_mb="$(awk '/MemAvailable:/ {print int($2/1024)}' /proc/meminfo 2>/dev/null || echo 0)"
    if [ "$mem_avail_mb" -lt 512 ]; then
        warn "Low memory: ${mem_avail_mb} MiB available (recommend >= 512 MiB)."
    else
        good "Memory OK (${mem_avail_mb} MiB available)"
    fi

    # Network reachability (skip in offline mode)
    if [ -n "$SERVERKIT_OFFLINE_TARBALL" ]; then
        info "Offline tarball set; skipping network checks"
    elif [ "$USE_RELEASE" = "1" ] || [ -z "$TARGET_BRANCH" ]; then
        if ! curl -sfI "https://github.com" >/dev/null 2>&1; then
            halt "Cannot reach github.com. Set SERVERKIT_OFFLINE_TARBALL or fix network."
        fi
        good "Network reachability OK"
    fi

    # Current backend health (warn only; it may already be down)
    if curl -sf --max-time 5 http://127.0.0.1:5000/api/v1/system/health >/dev/null 2>&1; then
        good "Backend currently healthy"
    else
        warn "Backend is not currently responding on :5000 — will proceed anyway"
    fi
}

# ---------------------------------------------------------------------------
# Python virtual environment
# ---------------------------------------------------------------------------
locate_python() {
    local c v
    for c in python3.12 python3.11 python3; do
        if command -v "$c" &>/dev/null; then
            v="$($c -c 'import sys;print(".".join(map(str,sys.version_info[:2])))' 2>/dev/null || true)"
            if printf '%s\n%s' "3.11" "$v" | sort -C -V && \
               printf '%s\n%s' "$v" "3.12" | sort -C -V; then
                printf '%s' "$c"
                return 0
            fi
        fi
    done
    return 1
}

rebuild_virtualenv() {
    local target_dir="$1"
    step "Rebuilding Python virtual environment..."

    local py_bin
    py_bin="$(locate_python)" || halt "ServerKit requires Python 3.11 or 3.12."

    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would recreate venv at $target_dir using $py_bin"
        return 0
    fi

    rm -rf "$target_dir"
    "$py_bin" -m venv "$target_dir"
    # shellcheck source=/dev/null
    source "$target_dir/bin/activate"
    pip install --upgrade pip --quiet
    pip install -r "$target_dir/../backend/requirements.txt" --quiet
    pip install gunicorn gevent gevent-websocket --quiet
    good "Virtual environment rebuilt at $target_dir"
}

# Ensure the target directory has a usable venv. If a pre-built one exists, use
# it; otherwise rebuild from requirements.
require_venv() {
    local target_dir="$1"
    if [ -f "$target_dir/bin/activate" ] && [ -x "$target_dir/bin/python" ]; then
        good "Virtual environment ready at $target_dir"
        return 0
    fi
    warn "Virtual environment missing at $target_dir"
    rebuild_virtualenv "$target_dir"
}

# ---------------------------------------------------------------------------
# Database migration
# ---------------------------------------------------------------------------
migrate_database() {
    local work_dir="$1"
    local venv="$work_dir/venv"

    step "Running database migrations..."
    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would run flask db upgrade in $work_dir/backend"
        return 0
    fi

    # For SQLite installs, migrate the COPY we placed in the *new* slot, not the
    # database the .env points at via the /opt/serverkit symlink (which still
    # resolves to the live OLD slot until atomic_switch). Touching the old slot's
    # DB here would corrupt rollback: reverting the symlink would hand the old
    # code a database already upgraded to the new schema. Pointing the migration
    # at the slot-absolute path keeps the old slot's DB byte-for-byte intact.
    #
    # Subshell: the venv activation and the cd must not leak into the main
    # shell — the rest of the update keeps running from the caller's cwd.
    local slot_db="$work_dir/backend/instance/serverkit.db"
    if ! (
        # shellcheck source=/dev/null
        source "$venv/bin/activate"
        cd "$work_dir/backend"
        if grep -qE '^DATABASE_URL=sqlite' "$work_dir/.env" 2>/dev/null && [ -f "$slot_db" ]; then
            DATABASE_URL="sqlite:///$slot_db" FLASK_ENV=production flask db upgrade
        else
            # Non-SQLite (e.g. PostgreSQL): the DB is shared/external, so there
            # is no per-slot copy to isolate — migrate it directly.
            FLASK_ENV=production flask db upgrade
        fi
    ); then
        halt "Database migration failed. The previous installation is still active."
    fi
    good "Database migrated"
}

# ---------------------------------------------------------------------------
# Release download + checksum verification
# ---------------------------------------------------------------------------
# Prints exactly ONE line on stdout: the path to the verified tarball. The
# caller captures that stdout (tarball="$(download_release ...)"), so every
# bit of progress in here is routed to stderr — a step/good line on stdout
# would be captured into the "path" and break the tar that follows. stderr
# still reaches the terminal and the log (init_logging merges 2>&1 into tee).
download_release() {
    local version="$1"
    local arch output tmp_dir base_url checksum_url tarball_url

    case "$(uname -m)" in
        x86_64|amd64)  arch="amd64" ;;
        aarch64|arm64) arch="arm64" ;;
        *)             halt "Unsupported architecture: $(uname -m)" ;;
    esac

    if [ -n "$SERVERKIT_OFFLINE_TARBALL" ]; then
        [ -f "$SERVERKIT_OFFLINE_TARBALL" ] || halt "Offline tarball not found: $SERVERKIT_OFFLINE_TARBALL"
        echo "$SERVERKIT_OFFLINE_TARBALL"
        return 0
    fi

    if [ -n "$SERVERKIT_MIRROR_URL" ]; then
        base_url="$SERVERKIT_MIRROR_URL"
    else
        base_url="https://github.com/${GITHUB_REPO}/releases/download/${version}"
    fi

    tarball_url="${base_url}/serverkit-${version}-linux-${arch}.tar.gz"
    checksum_url="${base_url}/checksums.txt"
    tmp_dir="$(mktemp -d)"
    output="$tmp_dir/serverkit-${version}-linux-${arch}.tar.gz"

    step "Downloading release tarball (${arch})..." >&2
    curl -sfL "$tarball_url" -o "$output" || halt "Failed to download $tarball_url"

    step "Verifying checksum..." >&2
    if curl -sfL "$checksum_url" -o "$tmp_dir/checksums.txt"; then
        cd "$tmp_dir"
        if ! sha256sum -c <(grep "serverkit-${version}-linux-${arch}.tar.gz" checksums.txt) >/dev/null 2>&1; then
            halt "Checksum verification failed for release tarball."
        fi
        good "Checksum verified" >&2
    else
        warn "Could not download checksums.txt — skipping verification" >&2
    fi

    echo "$output"
}

# ---------------------------------------------------------------------------
# Blue/green directory management
# ---------------------------------------------------------------------------
ensure_bluegreen_layout() {
    # Convert legacy single-directory installs into the blue/green symlink layout.
    if [ -d "$INSTALL_DIR" ] && [ ! -L "$INSTALL_DIR" ]; then
        step "Migrating to blue/green layout..."
        if [ "$DRY_RUN" = "1" ]; then
            info "[dry-run] would move $INSTALL_DIR → $DIR_A and symlink $INSTALL_DIR → $DIR_A"
            return 0
        fi
        mv "$INSTALL_DIR" "$DIR_A"
        ln -s "$DIR_A" "$INSTALL_DIR"
        good "Migrated to blue/green layout"
    fi

    # Ensure both slots exist.
    if [ "$DRY_RUN" = "0" ]; then
        mkdir -p "$DIR_A" "$DIR_B"
    fi
}

atomic_switch() {
    local target="$1"
    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would switch symlink $INSTALL_DIR → $target"
        return 0
    fi
    ln -sfn "$target" "${INSTALL_DIR}.tmp"
    mv -Tf "${INSTALL_DIR}.tmp" "$INSTALL_DIR"
    good "Switched active install to $target"
}

# ---------------------------------------------------------------------------
# Backup
# ---------------------------------------------------------------------------
backup_current() {
    phase "Database Backup"
    mkdir -p "$BACKUP_DIR"

    local active db_file backup_file
    active="$(active_real_dir)"
    db_file="$active/backend/instance/serverkit.db"

    if [ -f "$db_file" ]; then
        backup_file="$BACKUP_DIR/serverkit-pre-upgrade-$(date +%Y%m%d-%H%M%S).db"
        run_or_dry cp "$db_file" "$backup_file"
        good "Database backed up to $backup_file"
    else
        warn "No SQLite database at $db_file — skipping DB backup"
    fi

    local tree_backup
    tree_backup="$BACKUP_DIR/serverkit-tree-$(date +%Y%m%d-%H%M%S)"
    if [ -d "$active" ]; then
        run_or_dry rsync -a --exclude=venv --exclude=backups --exclude=node_modules \
            "$active/" "$tree_backup/" 2>/dev/null || \
            run_or_dry cp -a "$active" "$tree_backup" 2>/dev/null || true
        good "Install tree backed up to $tree_backup"
    fi
}

# ---------------------------------------------------------------------------
# Carry user-installed plugins across a redeploy (#48).
#
# URL/registry/upload-installed extensions live as extracted dirs under
# backend/app/plugins/<slug> and frontend/src/plugins/<slug>. A fresh
# clone/tarball only contains the repo-tracked ones, so anything the user
# installed would silently vanish — copy forward every plugin dir the new
# tree doesn't already ship. Runs BEFORE the frontend build so carried
# frontends compile into the new bundle. The backend also has a boot-time
# repair pass as a backstop (plugin_service.repair_missing_plugins).
# ---------------------------------------------------------------------------
preserve_installed_plugins() {
    local src="$1" target="$2"
    local sub dir name
    for sub in backend/app/plugins frontend/src/plugins; do
        [ -d "$src/$sub" ] || continue
        for dir in "$src/$sub"/*/; do
            [ -d "$dir" ] || continue
            name="$(basename "$dir")"
            [ "$name" = "__pycache__" ] && continue
            if [ ! -e "$target/$sub/$name" ]; then
                mkdir -p "$target/$sub" 2>/dev/null || true
                if cp -a "$dir" "$target/$sub/$name" 2>/dev/null; then
                    info "Preserved installed plugin: $sub/$name"
                else
                    warn "Could not preserve plugin $sub/$name"
                fi
            fi
        done
    done
    return 0
}

# ---------------------------------------------------------------------------
# Deploy source into the next directory
# ---------------------------------------------------------------------------
deploy_source() {
    local target="$1"
    local branch="${2:-main}"

    phase "Updating Source"

    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would clone/pull $GITHUB_REPO:$branch into $target"
        return 0
    fi

    # Preserve .env and database across rewrites.
    local tmp_env tmp_db
    tmp_env="$(mktemp)"
    tmp_db="$(mktemp)"
    cp "$INSTALL_DIR/.env" "$tmp_env" 2>/dev/null || true
    cp "$INSTALL_DIR/backend/instance/serverkit.db" "$tmp_db" 2>/dev/null || true

    rm -rf "$target"
    git clone --depth 1 --branch "$branch" "https://github.com/${GITHUB_REPO}.git" "$target" \
        || halt "Failed to clone ${GITHUB_REPO}:$branch"

    cp "$tmp_env" "$target/.env" 2>/dev/null || true
    mkdir -p "$target/backend/instance"
    cp "$tmp_db" "$target/backend/instance/serverkit.db" 2>/dev/null || true
    rm -f "$tmp_env" "$tmp_db"

    preserve_installed_plugins "$INSTALL_DIR" "$target"

    chmod +x "$target/serverkit"
    chmod +x "$target/scripts/"*.sh 2>/dev/null || true

    good "Source updated to $branch in $target"
}

# ---------------------------------------------------------------------------
# Deploy release tarball into the next directory
# ---------------------------------------------------------------------------
deploy_release() {
    local target="$1"
    local version="$2"

    phase "Downloading Release"

    local tarball stage unpacked
    tarball="$(download_release "$version")"

    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would unpack $tarball into $target"
        return 0
    fi

    stage="$(mktemp -d)"
    tar xzf "$tarball" -C "$stage"

    unpacked="$stage/serverkit"
    [ ! -d "$unpacked" ] && unpacked="$stage/opt/serverkit"
    if [ ! -d "$unpacked" ]; then
        # `|| true`: head exiting first can SIGPIPE find (rc 141 under
        # pipefail); an empty result is handled by the halt just below.
        unpacked="$(find "$stage" -maxdepth 2 -type d -name serverkit | head -n1 || true)"
    fi
    [ -d "$unpacked" ] || halt "Release tarball layout is unrecognized"

    # Preserve live state.
    cp "$INSTALL_DIR/.env" "$unpacked/.env" 2>/dev/null || true
    mkdir -p "$unpacked/backend/instance"
    cp "$INSTALL_DIR/backend/instance/serverkit.db" "$unpacked/backend/instance/serverkit.db" 2>/dev/null || true
    preserve_installed_plugins "$INSTALL_DIR" "$unpacked"

    rm -rf "$target"
    cp -a "$unpacked" "$target"
    rm -rf "$stage"

    chmod +x "$target/serverkit"
    chmod +x "$target/scripts/"*.sh 2>/dev/null || true

    good "Release $version deployed to $target"
}

# ---------------------------------------------------------------------------
# Configuration refresh
# ---------------------------------------------------------------------------
refresh_config() {
    local target="$1"

    phase "Refreshing Configuration"

    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would refresh nginx + systemd configs from $target"
        return 0
    fi

    mkdir -p "$NGINX_DIR/sites-available" "$NGINX_DIR/sites-enabled"

    # Recover panel domain from live nginx config. The trailing `|| true` keeps a
    # no-match/missing-file grep (exit 1/2) from tripping `set -euo pipefail` and
    # silently killing the updater before the atomic switch — HTTP-only boxes have
    # no serverkit.conf, so this grep finds nothing.
    local prior_panel_domain=""
    prior_panel_domain=$(grep -oE "$LETSENCRYPT_DIR/live/[^/]+/" \
        "$NGINX_DIR/sites-available/serverkit.conf" 2>/dev/null | head -n1 | \
        sed -E 's|.*/live/([^/]+)/|\1|' || true)
    [ "$prior_panel_domain" = "YOUR_DOMAIN" ] && prior_panel_domain=""

    if [ -f "$target/nginx/sites-available/serverkit.conf" ]; then
        cp "$target/nginx/sites-available/serverkit.conf" "$NGINX_DIR/sites-available/"
    fi
    if [ -f "$target/nginx/sites-available/serverkit-insecure.conf" ]; then
        cp "$target/nginx/sites-available/serverkit-insecure.conf" "$NGINX_DIR/sites-available/"
    fi

    # The panel frontend is served statically by host nginx from
    # $INSTALL_DIR/frontend/dist (the /opt/serverkit symlink). The shipped config
    # roots at the default /opt/serverkit; re-point it when SERVERKIT_DIR differs,
    # exactly as install.sh does, so a custom install dir survives upgrades.
    if [ "$INSTALL_DIR" != "/opt/serverkit" ]; then
        local conf
        for conf in serverkit.conf serverkit-insecure.conf; do
            [ -f "$NGINX_DIR/sites-available/$conf" ] && \
                sed -i "s|/opt/serverkit/frontend/dist|$INSTALL_DIR/frontend/dist|g" \
                    "$NGINX_DIR/sites-available/$conf"
        done
    fi

    # TLS floor — prefer a reversible conf.d snippet when nginx.conf doesn't
    # already declare these (a second declaration in the same http{} context is a
    # "duplicate ssl_protocols" error); otherwise rewrite in place. Mirrors
    # install.sh's harden_global_tls.
    if [ -f "$NGINX_DIR/nginx.conf" ]; then
        local ciphers='ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384'
        local has_proto=0 has_ciphers=0
        grep -qE '^[[:space:]]*ssl_protocols[[:space:]]' "$NGINX_DIR/nginx.conf" && has_proto=1
        grep -qE '^[[:space:]]*ssl_ciphers[[:space:]]'   "$NGINX_DIR/nginx.conf" && has_ciphers=1
        if [ "$has_proto" = "0" ] && [ "$has_ciphers" = "0" ] && \
           grep -qE 'include[[:space:]]+/etc/nginx/conf\.d/\*\.conf' "$NGINX_DIR/nginx.conf"; then
            mkdir -p "$NGINX_DIR/conf.d"
            cat > "$NGINX_DIR/conf.d/serverkit-tls.conf" <<EOF
# Auto-generated by ServerKit — server-wide TLS floor. Safe to remove.
ssl_protocols TLSv1.2 TLSv1.3;
ssl_ciphers ${ciphers};
EOF
        else
            rm -f "$NGINX_DIR/conf.d/serverkit-tls.conf" 2>/dev/null || true
            if [ "$has_proto" = "1" ]; then
                sed -i -E 's|^([[:space:]]*)ssl_protocols[[:space:]].*|\1ssl_protocols TLSv1.2 TLSv1.3;|' "$NGINX_DIR/nginx.conf"
            else
                sed -i '/http {/a \    ssl_protocols TLSv1.2 TLSv1.3;' "$NGINX_DIR/nginx.conf"
            fi
            if [ "$has_ciphers" = "1" ]; then
                sed -i -E "s|^([[:space:]]*)ssl_ciphers[[:space:]].*|\1ssl_ciphers ${ciphers};|" "$NGINX_DIR/nginx.conf"
            else
                sed -i "/http {/a \\    ssl_ciphers ${ciphers};" "$NGINX_DIR/nginx.conf"
            fi
        fi
    fi

    # SSL mode
    local ssl_mode="insecure"
    if [ -f "$CONFIG_DIR/ssl-mode" ]; then
        ssl_mode="$(cat "$CONFIG_DIR/ssl-mode")"
    fi
    if [ "$ssl_mode" = "secure" ] && [ -f "$NGINX_DIR/sites-available/serverkit.conf" ]; then
        local panel_domain=""
        if [ -f "$CONFIG_DIR/panel-domain" ]; then
            panel_domain="$(cat "$CONFIG_DIR/panel-domain" 2>/dev/null || true)"
        fi
        [ -z "$panel_domain" ] && panel_domain="$prior_panel_domain"

        if [ -n "$panel_domain" ] && [ -d "$LETSENCRYPT_DIR/live/$panel_domain" ]; then
            sed -i "s|/etc/letsencrypt/live/YOUR_DOMAIN/|$LETSENCRYPT_DIR/live/$panel_domain/|g" \
                "$NGINX_DIR/sites-available/serverkit.conf"
            ln -sf "$NGINX_DIR/sites-available/serverkit.conf" "$NGINX_DIR/sites-enabled/serverkit.conf"
        else
            warn "SSL mode is 'secure' but no certificate found for '${panel_domain:-unknown}'"
            ln -sf "$NGINX_DIR/sites-available/serverkit-insecure.conf" "$NGINX_DIR/sites-enabled/serverkit.conf"
        fi
    else
        ln -sf "$NGINX_DIR/sites-available/serverkit-insecure.conf" "$NGINX_DIR/sites-enabled/serverkit.conf"
    fi

    # Service unit — render from the template so a custom SERVERKIT_DIR / venv /
    # log path survives upgrades (the unit references the /opt/serverkit symlink,
    # so blue/green switches need no re-render). Fall back to a plain unit if an
    # older tree still ships one.
    local svc_template="$target/templates/serverkit-backend.service.in"
    if [ -f "$svc_template" ]; then
        sed -e "s|@SERVERKIT_DIR@|$INSTALL_DIR|g" \
            -e "s|@SERVERKIT_VENV_DIR@|$VENV_DIR|g" \
            -e "s|@PORT@|5000|g" \
            -e "s|@USER@|root|g" \
            -e "s|@LOG_DIR@|$LOG_DIR|g" \
            "$svc_template" > "$SYSTEMD_DIR/serverkit.service"
    elif [ -f "$target/serverkit-backend.service" ]; then
        cp "$target/serverkit-backend.service" "$SYSTEMD_DIR/serverkit.service"
    fi
    systemctl daemon-reload 2>/dev/null \
        || warn "systemd daemon-reload failed — the backend may restart with a stale unit"

    report_stale_panel_vhosts
    good "Configuration refreshed"
}

# The panel used to be served by a frontend container on :3847; it is now
# served statically by host nginx. A leftover custom vhost proxying to :3847
# keeps serving whatever bundle the (never-updated) container holds — the
# panel looks "stuck on an old version" for that hostname only, which reads
# like a browser/CDN cache problem and wastes hours (2026-07-03 incident).
# Observation only (R1): warn and continue, tolerate a missing/empty dir.
report_stale_panel_vhosts() {
    local hits=""
    hits="$(grep -rls 'proxy_pass http://127\.0\.0\.1:3847' \
        "$NGINX_DIR/sites-enabled/" 2>/dev/null | sort -u | tr '\n' ' ' || true)"
    if [ -n "$hits" ]; then
        warn "Vhost(s) still proxy the panel to the retired :3847 frontend container: ${hits}"
        warn "  These serve a STALE panel bundle. Repoint them at the static SPA"
        warn "  (root $INSTALL_DIR/frontend/dist + /api and /socket.io → :5000),"
        warn "  mirroring sites-available/serverkit.conf."
    fi
}

# ---------------------------------------------------------------------------
# Rollback
# ---------------------------------------------------------------------------
# Probe the backend health endpoint; returns 0 if healthy within <timeout>s.
# Side-effect free (unlike health_check, which triggers a rollback) so it is
# safe to call *after* a rollback to confirm the restored version came back.
quick_health() {
    local timeout="${1:-30}" waited=0
    while [ "$waited" -lt "$timeout" ]; do
        curl -sf --max-time 5 http://127.0.0.1:5000/api/v1/system/health >/dev/null 2>&1 && return 0
        sleep 2
        waited=$((waited + 2))
    done
    return 1
}

rollback() {
    # Flag FIRST: halt() below exits the script, which fires the EXIT trap
    # (cleanup_on_exit) — without this flag already set, that trap would run a
    # second, redundant rollback on top of the one that just finished.
    ROLLING_BACK=1
    warn "Update failed — rolling back to previous slot..."

    if [ -z "${PREVIOUS_DIR:-}" ] || [ ! -d "$PREVIOUS_DIR" ]; then
        halt "Cannot roll back: previous installation directory not available"
    fi

    # Same zero-downtime discipline as the forward path: cycle only the backend,
    # never stop nginx (it fronts every hosted app), and reload its config after
    # the slot is switched back.
    systemctl stop "$BACKEND_SERVICE" 2>/dev/null || true
    wait_for_service "$BACKEND_SERVICE" inactive 30 || true

    atomic_switch "$PREVIOUS_DIR"

    # A rollback must never abort mid-flight over one failed step — everything
    # from here on is best-effort so the backend still gets started.
    systemctl daemon-reload 2>/dev/null \
        || warn "systemd daemon-reload failed during rollback — continuing"
    systemctl start "$BACKEND_SERVICE" 2>/dev/null || true
    wait_for_service "$BACKEND_SERVICE" active 30 || true
    # Frontend is static (served from the restored slot); just re-label + reload.
    selinux_label_dist "$PREVIOUS_DIR"
    reload_nginx_graceful

    # Confirm the restored version actually answers — a rollback that itself
    # comes up unhealthy is a far worse state to leave the operator guessing in.
    if quick_health 30; then
        halt "Rolled back to $(active_real_dir) and it is healthy. Inspect logs: journalctl -u serverkit -n 50"
    else
        halt "Rolled back to $(active_real_dir) but it is STILL UNHEALTHY — manual intervention needed. Logs: journalctl -u serverkit -n 50"
    fi
}

# If the update fails after we have switched the symlink, roll back to the
# previous slot automatically. Registered as the EXIT trap by the run block
# below; defined here (above the source guard) so it stays unit-testable.
cleanup_on_exit() {
    local rc=$?
    [ "$rc" -eq 0 ] && return 0
    if [ "$DRY_RUN" = "0" ] && [ -n "${PREVIOUS_DIR:-}" ] && \
       [ "${ROLLING_BACK:-0}" != "1" ] && [ "${HEALTH_PASSED:-0}" != "1" ]; then
        ROLLING_BACK=1
        rollback
    fi
    # Always return 0: the script's exit code is already $rc (an EXIT trap does
    # not change it) — returning $rc here only re-fires the ERR trap, which
    # appends a second, misleading "Update aborted" report.
    return 0
}

# ---------------------------------------------------------------------------
# Health check
# ---------------------------------------------------------------------------
health_check() {
    phase "Health Check"

    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would probe http://127.0.0.1:5000/api/v1/system/health"
        return 0
    fi

    step "Waiting for backend..."
    local waited=0
    while [ "$waited" -lt 60 ]; do
        if curl -sf --max-time 5 http://127.0.0.1:5000/api/v1/system/health >/dev/null 2>&1; then
            good "Backend healthy"
            break
        fi
        sleep 2
        waited=$((waited + 2))
    done
    if [ "$waited" -ge 60 ]; then
        rollback
    fi

    if ! curl -sf --max-time 5 http://127.0.0.1:5000/api/v1/system/health >/dev/null 2>&1; then
        rollback
    fi

    # No frontend probe: the panel frontend is static files served by host
    # nginx from the active slot — there is no frontend container anymore.
    HEALTH_PASSED=1
}

# ---------------------------------------------------------------------------
# Cleanup
# ---------------------------------------------------------------------------
# Trim a family of backups down to its newest <keep> entries. Deliberately
# find-based: the old `ls -t "$BACKUP_DIR"/<glob> | tail | xargs` aborted the
# whole updater under `set -Eeuo pipefail` whenever the glob matched nothing
# (ls exits 2) — i.e. on every box with no prior backups of that family, AFTER
# an otherwise successful update. find treats "nothing matched" as a clean
# empty result, and an absent BACKUP_DIR is an explicit no-op.
prune_old_backups() {
    local pattern="$1" keep="$2"
    [ -d "$BACKUP_DIR" ] || return 0
    find "$BACKUP_DIR" -mindepth 1 -maxdepth 1 -name "$pattern" -printf '%T@\t%p\n' 2>/dev/null \
        | sort -rn | tail -n +"$((keep + 1))" | cut -f2- \
        | xargs -r -d '\n' rm -rf || true
}

cleanup() {
    phase "Cleanup"

    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would trim old backups"
        return 0
    fi

    prune_old_backups 'serverkit-tree-*'           10
    prune_old_backups 'serverkit-pre-upgrade-*.db' 10

    local new_version
    new_version="$(cat "$INSTALL_DIR/VERSION" 2>/dev/null | tr -d '\n\r ' || echo "unknown")"
    curl -s "https://serverkit.ai/track/update?v=${new_version}" >/dev/null 2>&1 || true
    good "Cleanup complete"
}

# ---------------------------------------------------------------------------
# Firewall — keep 80/443 open across updates.
# ---------------------------------------------------------------------------
# Idempotent and best-effort: a box installed before firewall automation existed
# gets its ports opened on the next update; an already-configured box is a no-op.
# Records what we opened in install-state.json so uninstall can undo it. Sources
# the helpers from the (now-current) install tree, which carries scripts/lib.
ensure_firewall() {
    local lib="$INSTALL_DIR/scripts/lib"
    [ -f "$lib/firewall.sh" ] || return 0
    # shellcheck source=/dev/null
    source "$lib/firewall.sh"
    if [ -f "$lib/state.sh" ]; then
        # shellcheck source=/dev/null
        source "$lib/state.sh"
    fi

    local backend
    backend="$(firewall_detect)"
    if [ "$backend" = "none" ]; then
        return 0
    fi

    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would ensure firewall ($backend) allows 80/tcp and 443/tcp"
        return 0
    fi

    firewall_open "$backend" 80/tcp 443/tcp || true
    if command -v state_set >/dev/null 2>&1; then
        state_set firewall_backend "$backend" || true
        state_append firewall_ports 80/tcp || true
        state_append firewall_ports 443/tcp || true
    fi
    good "Firewall ensured ($backend): 80/tcp and 443/tcp open"
}

# ---------------------------------------------------------------------------
# All-Docker deployment (compose) detection + update path
# ---------------------------------------------------------------------------
# A ServerKit box runs in one of two shapes:
#   * Hybrid (canonical): backend in a host venv under systemd `serverkit`,
#     only the frontend in Docker. The blue/green + venv + systemd flow below
#     targets this shape.
#   * All-Docker: both backend AND frontend run as docker-compose services
#     (container_name serverkit-backend / serverkit-frontend), no host venv.
# Running the hybrid flow on an all-Docker box builds a useless host venv,
# migrates the wrong (non-container) database, and stops a systemd unit that
# never serves traffic — which is exactly the failure that ends in
# "/opt/serverkit/venv/bin/activate: No such file or directory". Detect the
# all-Docker shape and route it to a dedicated compose update instead.
is_docker_deployment() {
    [ -f "$INSTALL_DIR/docker-compose.yml" ] || return 1
    # A usable host venv means this is the hybrid shape — take precedence.
    [ -x "$INSTALL_DIR/venv/bin/python" ] && return 1
    # The backend container is defined by the all-Docker compose project.
    docker ps -a --format '{{.Names}}' 2>/dev/null | grep -qx 'serverkit-backend'
}

dc() {
    if docker compose version &>/dev/null; then
        ( cd "$INSTALL_DIR" && docker compose "$@" )
    else
        ( cd "$INSTALL_DIR" && docker-compose "$@" )
    fi
}

backup_docker_db() {
    mkdir -p "$BACKUP_DIR"
    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would copy the SQLite DB out of the backend container"
        return 0
    fi
    local backup_file
    backup_file="$BACKUP_DIR/serverkit-pre-upgrade-$(date +%Y%m%d-%H%M%S).db"
    # The live DB lives in the serverkit-data volume at /app/instance inside
    # the container, not in the source tree.
    if docker cp serverkit-backend:/app/instance/serverkit.db "$backup_file" 2>/dev/null; then
        good "Database backed up to $backup_file"
    else
        warn "Could not copy DB from container — continuing (backend self-backs-up before migrating)"
    fi
}

# L6 — Heal layout left by an interrupted or legacy (hybrid) run before the
# docker update touches anything.
heal_layout_for_docker() {
    # Pin the compose project name. A blue/green symlink otherwise makes compose
    # derive the project name from the link target and warn — worse, a changed
    # name would orphan the running containers.
    export COMPOSE_PROJECT_NAME="${COMPOSE_PROJECT_NAME:-$BASE_NAME}"

    [ "$DRY_RUN" = "1" ] && return 0

    # On an all-Docker box the blue/green slots are vestigial. If the install is
    # a symlink (a prior hybrid run migrated it), reclaim the *inactive* slot it
    # left behind — the live tree + DB volume are elsewhere, so this is safe.
    if [ -L "$INSTALL_DIR" ]; then
        local active inactive
        active="$(active_real_dir)"
        inactive="$DIR_A"; [ "$active" = "$DIR_A" ] && inactive="$DIR_B"
        if [ -d "$inactive" ]; then
            warn "Reclaiming stale slot from a prior run: $inactive"
            rm -rf "$inactive"
        fi
    fi
}

# L4 — Snapshot the current docker deployment so a bad upgrade can be reverted.
DOCKER_PREV_SHA=""
DOCKER_ROLLBACK_READY=0
snapshot_docker_state() {
    [ "$DRY_RUN" = "1" ] && return 0
    DOCKER_PREV_SHA="$(git -C "$INSTALL_DIR" rev-parse HEAD 2>/dev/null || true)"
    # Tag the currently-running images so we can re-point :latest back to them,
    # and stash the host-built bundle that the new build is about to overwrite.
    local img
    for img in serverkit-backend serverkit-frontend; do
        docker image inspect "$img:latest" >/dev/null 2>&1 && \
            docker tag "$img:latest" "$img:rollback" 2>/dev/null || true
    done
    if [ -d "$INSTALL_DIR/frontend/dist" ]; then
        rm -rf "$INSTALL_DIR/frontend/dist.rollback"
        cp -a "$INSTALL_DIR/frontend/dist" "$INSTALL_DIR/frontend/dist.rollback" 2>/dev/null || true
    fi
    DOCKER_ROLLBACK_READY=1
}

rollback_docker() {
    [ "$DOCKER_ROLLBACK_READY" = "1" ] || halt "New version is unhealthy and no rollback snapshot exists — inspect: cd $INSTALL_DIR && docker compose logs backend"
    warn "New version unhealthy — rolling back to the previous deployment..."

    # Guard the reset on a non-empty SHA: an empty SHA would make `git reset
    # --hard ''` reset to HEAD (a no-op at best, surprising at worst). The image
    # re-tag + bundle restore below still recover the previous deployment even
    # when no commit was recorded.
    if [ -n "$DOCKER_PREV_SHA" ]; then
        git -C "$INSTALL_DIR" reset --hard "$DOCKER_PREV_SHA" 2>&1 | tail -n2 || \
            warn "git reset to $DOCKER_PREV_SHA failed during rollback"
    else
        warn "No previous commit recorded — skipping git reset (images/bundle still restored)"
    fi
    if [ -d "$INSTALL_DIR/frontend/dist.rollback" ]; then
        rm -rf "$INSTALL_DIR/frontend/dist"
        mv "$INSTALL_DIR/frontend/dist.rollback" "$INSTALL_DIR/frontend/dist"
    fi
    local img
    for img in serverkit-backend serverkit-frontend; do
        docker image inspect "$img:rollback" >/dev/null 2>&1 && \
            docker tag "$img:rollback" "$img:latest" 2>/dev/null || true
    done
    dc up -d 2>&1 | tail -n10 || true

    # Confirm the restored deployment came back healthy.
    local waited=0 status=""
    while [ "$waited" -lt 60 ]; do
        status="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' serverkit-backend 2>/dev/null || echo missing)"
        { [ "$status" = "healthy" ] || [ "$status" = "none" ]; } && break
        sleep 3
        waited=$((waited + 3))
    done
    if [ "$status" = "healthy" ] || [ "$status" = "none" ]; then
        halt "Rolled back to previous version (${DOCKER_PREV_SHA:-unknown}) and it is healthy. Logs: cd $INSTALL_DIR && docker compose logs backend"
    else
        halt "Rolled back to previous version (${DOCKER_PREV_SHA:-unknown}) but it is STILL UNHEALTHY (status: $status) — manual intervention needed. Logs: cd $INSTALL_DIR && docker compose logs backend"
    fi
}

# Drop rollback artifacts once an upgrade is confirmed healthy.
clear_docker_rollback() {
    [ "$DRY_RUN" = "1" ] && return 0
    rm -rf "$INSTALL_DIR/frontend/dist.rollback" 2>/dev/null || true
    local img
    for img in serverkit-backend serverkit-frontend; do
        docker image inspect "$img:rollback" >/dev/null 2>&1 && \
            docker rmi "$img:rollback" >/dev/null 2>&1 || true
    done
}

update_docker_compose() {
    printf '\n  %s%sServerKit Updater — Docker deployment%s\n' "$BLD" "$PAPER" "$RST"
    STARTED_AT=$(date +%s)
    [ "$DRY_RUN" = "1" ] && warn "DRY RUN — no changes will be made"

    command -v docker &>/dev/null || halt "docker is required but not installed"
    command -v git &>/dev/null || halt "git is required but not installed"

    heal_layout_for_docker

    # Resolve the git ref to update to (branch / release tag / main).
    local ref="origin/main"
    local mode="main"
    if [ -n "$TARGET_BRANCH" ]; then
        ref="origin/$TARGET_BRANCH"
        mode="branch:$TARGET_BRANCH"
    elif [ "$USE_RELEASE" = "1" ]; then
        # `|| true`: head exiting first can SIGPIPE git (rc 141 under
        # pipefail); an empty ref is caught by the halt just below.
        ref="${RELEASE_VERSION:-$(git -C "$INSTALL_DIR" tag -l 'v*' --sort=-v:refname | head -n1 || true)}"
        [ -n "$ref" ] || halt "Could not determine a release tag to update to"
        mode="release"
    fi

    # Skip when already current (unless --force). Capture the starting version
    # for the summary before we touch the tree.
    local old_version
    old_version="$(local_version)"
    version_gate

    phase "Database Backup"
    backup_docker_db

    # Snapshot the current (still-running, last-known-good) deployment before we
    # mutate the tree, rebuild the bundle, or rebuild images — so an unhealthy
    # upgrade can be reverted.
    snapshot_docker_state

    phase "Syncing Source"
    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would git fetch and reset $INSTALL_DIR to $ref"
    else
        # The compose build context IS the live tree, so update it in place
        # (no blue/green). Untracked .env, instance DB and frontend/dist are
        # preserved across a hard reset.
        git -C "$INSTALL_DIR" fetch --all --tags --prune 2>&1 | tail -n2 || halt "git fetch failed"
        git -C "$INSTALL_DIR" reset --hard "$ref" 2>&1 | tail -n2 || halt "git reset to $ref failed"
        chmod +x "$INSTALL_DIR/serverkit" "$INSTALL_DIR/scripts/"*.sh 2>/dev/null || true
        good "Source synced to $ref"
    fi

    # When the compose serves a host-built bundle (bind-mounted ./frontend/dist)
    # the image's own assets are shadowed, so the bundle must be rebuilt on the
    # host. Otherwise the frontend image carries its assets and needs no host build.
    if grep -qE '\./frontend/dist[ :]' "$INSTALL_DIR/docker-compose.yml" 2>/dev/null; then
        phase "Building Frontend"
        if [ "$DRY_RUN" = "1" ]; then
            info "[dry-run] would npm ci + npm run build in frontend/"
        elif command -v npm &>/dev/null; then
            ( cd "$INSTALL_DIR/frontend" && npm ci 2>&1 | tail -n3 && \
              NODE_OPTIONS="--max-old-space-size=1024" npm run build 2>&1 | tail -n5 ) \
              || halt "Frontend build failed"
            good "Frontend assets rebuilt"
        else
            warn "npm not on host; relying on the frontend image build for assets"
        fi
    fi

    phase "Building Images"
    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would docker compose build"
    else
        # Build before recreating: running containers are untouched until
        # 'up -d', so a failed build leaves the current version serving.
        dc build 2>&1 | tail -n15 || halt "docker compose build failed — current version still running"
        good "Images built"
    fi

    phase "Recreating Containers"
    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would docker compose up -d (backend auto-migrates on boot)"
    else
        dc up -d 2>&1 | tail -n15 || halt "docker compose up -d failed"
        # If the frontend serves a host-built bundle, restart it so nginx picks
        # up freshly built assets even when its own image/config did not change.
        if grep -qE '\./frontend/dist[ :]' "$INSTALL_DIR/docker-compose.yml" 2>/dev/null; then
            dc restart frontend 2>&1 | tail -n3 || true
        fi
        good "Containers recreated"
    fi

    phase "Health Check"
    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would wait for serverkit-backend to report healthy"
    else
        local waited=0 status=""
        while [ "$waited" -lt 90 ]; do
            status="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' serverkit-backend 2>/dev/null || echo missing)"
            [ "$status" = "healthy" ] && { good "Backend healthy"; break; }
            [ "$status" = "none" ] && { good "Backend container running (no healthcheck defined)"; break; }
            sleep 3
            waited=$((waited + 3))
        done
        if [ "$status" != "healthy" ] && [ "$status" != "none" ]; then
            warn "Backend did not report healthy within 90s (status: $status)"
            rollback_docker   # L4 — revert to the snapshot; this halts the script
        fi
    fi

    # Healthy (or no healthcheck defined): the upgrade stuck, drop the snapshot.
    clear_docker_rollback

    # Keep the firewall in sync (idempotent, best-effort).
    ensure_firewall || true

    local new_version
    new_version="$(cat "$INSTALL_DIR/VERSION" 2>/dev/null | tr -d '\n\r ' || echo unknown)"
    curl -s "https://serverkit.ai/track/update?v=${new_version}" >/dev/null 2>&1 || true

    printf '\n  %s%s✔  Update complete (Docker)%s   %s%s%s\n\n' \
        "$BLD" "$HUE_OK" "$RST" "$FOG" "$(clock)" "$RST"
    printf '  Updated   %s → %s\n' "${old_version:-unknown}" "$new_version"
    printf '  Mode      docker/%s\n' "$mode"
    printf '  Duration  %s\n' "$(clock)"
    printf '  Backend   %s\n' "$(docker inspect -f '{{.State.Status}}' serverkit-backend 2>/dev/null || echo unknown)"
    printf '  Frontend  %s\n' "$(docker inspect -f '{{.State.Status}}' serverkit-frontend 2>/dev/null || echo unknown)"
    [ -n "${UPDATE_LOG:-}" ] && printf '  Log       %s\n' "$UPDATE_LOG"
    printf '\n  %sCLI%s       serverkit status\n\n' "$BLD" "$RST"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

# Sourcing this file (e.g. from scripts/test/test_update.sh) exposes every
# function above for unit testing without running an update. Only a direct
# execution falls through to the run below.
[ "${BASH_SOURCE[0]}" = "${0}" ] || return 0

maybe_reexec_latest_updater   # may exec into the newest updater and not return
acquire_update_lock
init_logging
# L5 — turn any unguarded failure (in the main flow OR a helper, thanks to -E)
# into a labelled report instead of a silent drop back to the prompt.
trap 'report_failure "$?" "$LINENO" "$BASH_COMMAND"' ERR

# All-Docker deployments don't use the host venv / systemd / blue-green flow —
# route them to the compose path. This is what prevents the missing-venv crash.
if is_docker_deployment; then
    update_docker_compose
    exit 0
fi

printf '\n  %s%sServerKit Updater%s\n' "$BLD" "$PAPER" "$RST"
STARTED_AT=$(date +%s)

[ "$DRY_RUN" = "1" ] && warn "DRY RUN — no changes will be made"

# Roll back automatically if the update fails after the symlink switch (the
# handler, cleanup_on_exit, is defined next to rollback() above).
trap cleanup_on_exit EXIT

preflight_check

# Capture where we are coming from and what mode this is, then skip the whole
# update when the box is already current (unless --force). Doing this before any
# backup/deploy work means an up-to-date box does zero churn.
OLD_VERSION="$(local_version)"
if [ "$USE_RELEASE" = "1" ]; then
    UPDATE_MODE="release"
elif [ -n "$TARGET_BRANCH" ]; then
    UPDATE_MODE="branch:$TARGET_BRANCH"
else
    UPDATE_MODE="main"
fi
version_gate

ensure_bluegreen_layout
backup_current

# Determine update mode.
NEXT_DIR="$(next_real_dir)"

if [ "$USE_RELEASE" = "1" ]; then
    if [ -z "$RELEASE_VERSION" ]; then
        RELEASE_VERSION="$(curl -sf "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" \
            | grep '"tag_name"' | head -1 | cut -d'"' -f4 || true)"
        [ -n "$RELEASE_VERSION" ] || halt "Could not determine the latest release"
    fi
    step "Updating to release $RELEASE_VERSION"
    deploy_release "$NEXT_DIR" "$RELEASE_VERSION"
elif [ -n "$TARGET_BRANCH" ]; then
    step "Updating to branch $TARGET_BRANCH"
    deploy_source "$NEXT_DIR" "$TARGET_BRANCH"
else
    deploy_source "$NEXT_DIR" "main"
fi

# Ensure venv in the new tree.
require_venv "$NEXT_DIR/venv"

# Run database migrations before switching.
migrate_database "$NEXT_DIR"

# Sync templates.
if [ "$DRY_RUN" = "0" ]; then
    mkdir -p /etc/serverkit/templates
    cp -r "$NEXT_DIR/backend/templates/"*.yaml /etc/serverkit/templates/ 2>/dev/null || true
    cp -r "$NEXT_DIR/backend/templates/"*.yml  /etc/serverkit/templates/ 2>/dev/null || true
fi

# Build frontend if dist is missing (source mode or older release).
if [ ! -d "$NEXT_DIR/frontend/dist" ]; then
    step "Building frontend..."
    if [ "$DRY_RUN" = "0" ]; then
        # Subshell so the cd cannot leak into the main shell; the pipeline is
        # guarded so a failed build halts loudly while the old slot still serves.
        ( cd "$NEXT_DIR/frontend" && \
          npm ci --prefer-offline 2>&1 | tail -3 && \
          NODE_OPTIONS="--max-old-space-size=1024" npm run build 2>&1 | tail -5 ) \
            || halt "Frontend build failed — previous installation still active"
    else
        info "[dry-run] would npm ci + npm run build in $NEXT_DIR/frontend"
    fi
fi

# Refresh nginx/systemd configs in the active tree before switch.
refresh_config "$NEXT_DIR"

# Stop services.
#
# nginx is deliberately NOT stopped: it fronts every managed app, so taking it
# down would black out unrelated sites for the whole switch. Only the panel
# backend is cycled (a brief panel-API gap that never touches hosted apps), and
# nginx picks up any refreshed config via a graceful reload after the switch.
#
# First snapshot which hosted apps are reachable through nginx right now, so we
# can prove afterwards that the update did not knock any of them offline.
APP_BASELINE="$(snapshot_app_reachability)"

phase "Stopping Services"
# Guarded like the rollback path: `systemctl stop` exits non-zero when the unit
# is not loaded (exit 5) — a reason to warn, never to abort a healthy update.
run_or_dry systemctl stop "$BACKEND_SERVICE" \
    || warn "Backend service did not stop cleanly (unit may not be loaded) — continuing"
wait_for_service "$BACKEND_SERVICE" inactive 30 || warn "Backend did not stop within 30 seconds"

# Record the currently active directory before switching.
PREVIOUS_DIR="$(active_real_dir)"

# Atomic switch.
atomic_switch "$NEXT_DIR"

# Start services.
#
# The frontend is now static files under the switched-in slot's frontend/dist,
# served directly by host nginx (no container to recreate). The atomic switch
# already swapped the served assets; nginx only needs a graceful config reload.
phase "Starting Services"
run_or_dry systemctl start "$BACKEND_SERVICE"
wait_for_service "$BACKEND_SERVICE" active 30 || warn "Backend did not report active within 30 seconds"
# Re-label the now-active slot's bundle for SELinux hosts before nginx serves it.
selinux_label_dist "$NEXT_DIR"
# Graceful, zero-downtime config swap — never a stop/start (see reload_nginx_graceful).
reload_nginx_graceful
good "Services started"

# Health check.
health_check

# Verify the panel update did not take any hosted app down. Compare the
# pre-switch reachability snapshot with a fresh probe now that nginx has
# reloaded. Best-effort: a regression is reported loudly but does not fail the
# (already-healthy) update — the operator decides what to do about an app that
# was likely already unhealthy.
if [ "$DRY_RUN" = "0" ]; then
    phase "Verifying App Uptime"
    APP_AFTER="$(snapshot_app_reachability)"
    report_app_uptime_regressions "$APP_BASELINE" "$APP_AFTER" || true
fi

# Keep the firewall in sync (idempotent, best-effort).
ensure_firewall || true

# Cleanup.
cleanup

# Summary.
NEW_VERSION="$(local_version)"
printf '\n  %s%s✔  Update complete%s   %s%s%s\n\n' \
    "$BLD" "$HUE_OK" "$RST" "$FOG" "$(clock)" "$RST"
printf '  Updated   %s → %s\n' "${OLD_VERSION:-unknown}" "${NEW_VERSION:-unknown}"
printf '  Mode      %s\n' "${UPDATE_MODE:-main}"
printf '  Duration  %s\n' "$(clock)"
printf '  Active    %s\n' "$(active_real_dir)"
printf '  Backend   %s\n' "$(systemctl is-active serverkit 2>/dev/null || echo unknown)"
printf '  Nginx     %s\n\n' "$(systemctl is-active nginx 2>/dev/null || echo unknown)"
[ -n "${UPDATE_LOG:-}" ] && printf '  Log       %s\n\n' "$UPDATE_LOG"
printf '  %sCLI%s       serverkit status\n\n' "$BLD" "$RST"
