# shellcheck shell=bash
#
# scripts/lib/uninstall.sh — the one canonical uninstall routine.
#
# Both entry points (the standalone uninstall.sh and `serverkit uninstall`) used
# to carry their own copy of the teardown logic and had quietly drifted apart —
# `serverkit uninstall` ran `docker compose down -v` (destroying volumes) while
# uninstall.sh ran `down` (keeping them), so the same command name deleted your
# data in one path and not the other. This module is the single source of truth;
# the callers only handle their own banner + confirmation, then call
# serverkit_uninstall_core. It is non-interactive (testable) and parameterized
# entirely through environment variables.
#
# Inputs (all optional):
#   SERVERKIT_DIR        install dir (default /opt/serverkit)
#   SERVERKIT_PURGE      1 → also remove Docker volumes, the SQLite DB, and all
#                            data dirs (/var/lib, /var/serverkit, backups, /etc)
#   SERVERKIT_KEEP_DATA  1 → preserve /var/lib/serverkit, /etc/serverkit and
#                            /var/backups/serverkit (overrides the default
#                            config-dir removal). Ignored when PURGE=1.
#   SERVERKIT_UNINSTALL_DRY_RUN  1 → print actions, change nothing.
#
# Data-handling matrix:
#                                   default   --keep-data   --purge
#   code (install dir + slots)      remove    remove        remove
#   systemd/nginx/apt/CLI/firewall  remove    remove        remove
#   logs (/var/log/serverkit)       remove    remove        remove
#   Docker volumes                  keep      keep          remove (-v)
#   /etc/serverkit (config)         remove    keep          remove
#   /var/lib/serverkit              keep      keep          remove
#   /var/serverkit (apps)           keep      keep          remove
#   /var/backups/serverkit          keep      keep          remove
#   live SQLite DB                  backed up backed up     remove
#
# "Default preserves data" is honored for both deployment shapes: all-Docker DBs
# live in a volume (kept without -v); hybrid DBs live in the install dir, so we
# snapshot the DB into /var/backups/serverkit before removing the tree.

# Self-contained logging (no dependency on the caller's color helpers).
_u_info() { printf '  \033[0;34m→\033[0m %s\n' "$1"; }
_u_ok()   { printf '  \033[0;32m✓\033[0m %s\n' "$1"; }
_u_warn() { printf '  \033[1;33m!\033[0m %s\n' "$1"; }

_u_run() {
    if [ "${SERVERKIT_UNINSTALL_DRY_RUN:-0}" = "1" ]; then
        printf '  [uninstall] would run: %s\n' "$*"
        return 0
    fi
    "$@"
}

# Best-effort teardown step (rule R7): run it and, on failure, warn and keep
# going. By the time these steps run the services are already stopped, so an
# uninstall must never die mid-teardown and strand the box half-removed over a
# single EBUSY (live bind-mount), EROFS or ENOSPC rm/mkdir.
_u_try() {
    if _u_run "$@"; then
        return 0
    fi
    _u_warn "Step failed (continuing): $*"
    return 0
}

# Remove the firewall rules we (and only we) added, reading the record left in
# install-state.json. Must run BEFORE the config dir is deleted.
_u_firewall_cleanup() {
    local lib="$1/scripts/lib"
    [ -f "$lib/firewall.sh" ] || return 0
    [ -f "$lib/state.sh" ] || return 0
    # shellcheck source=/dev/null
    source "$lib/firewall.sh"
    # shellcheck source=/dev/null
    source "$lib/state.sh"
    command -v state_get >/dev/null 2>&1 || return 0

    local backend ports
    backend="$(state_get firewall_backend 2>/dev/null || true)"
    [ -n "$backend" ] || return 0
    # `|| true` mirrors the state_get line above: under the entry points'
    # pipefail a failing state_list would otherwise abort the WHOLE uninstall
    # before anything was removed.
    ports="$(state_list firewall_ports 2>/dev/null | tr '\n' ' ' || true)"
    [ -n "$ports" ] || return 0

    _u_info "Removing firewall rules we added ($backend): $ports"
    if [ "${SERVERKIT_UNINSTALL_DRY_RUN:-0}" = "1" ]; then
        FW_DRY_RUN=1 firewall_close "$backend" $ports
    else
        # shellcheck disable=SC2086
        firewall_close "$backend" $ports || true
    fi
}

serverkit_uninstall_core() {
    local install_dir="${SERVERKIT_DIR:-/opt/serverkit}"
    local purge="${SERVERKIT_PURGE:-0}"
    local keep_data="${SERVERKIT_KEEP_DATA:-0}"
    [ "$purge" = "1" ] && keep_data=0   # purge wins; keeping data makes no sense

    local data_dir="/var/lib/serverkit"
    local apps_dir="/var/serverkit"
    local config_dir="/etc/serverkit"
    local log_dir="/var/log/serverkit"
    local backup_dir="/var/backups/serverkit"

    # ---- Stop services -----------------------------------------------------
    _u_info "Stopping services..."
    _u_run systemctl stop serverkit 2>/dev/null || true
    _u_run systemctl disable serverkit 2>/dev/null || true
    _u_ok "Backend service stopped"

    # ---- Snapshot the live DB (unless purging) -----------------------------
    if [ "$purge" != "1" ]; then
        _u_snapshot_db "$install_dir" "$backup_dir"
    fi

    # ---- Containers --------------------------------------------------------
    if command -v docker >/dev/null 2>&1 && \
       { [ -e "$install_dir/docker-compose.yml" ] || [ -e "$install_dir/docker-compose.yaml" ]; }; then
        if [ "$purge" = "1" ]; then
            _u_info "Removing containers and volumes (--purge)..."
            _u_run docker compose --project-directory "$install_dir" down -v --remove-orphans 2>/dev/null || true
        else
            _u_info "Removing containers (volumes preserved)..."
            _u_run docker compose --project-directory "$install_dir" down --remove-orphans 2>/dev/null || true
        fi
        _u_ok "Containers removed"
    else
        _u_warn "Docker or compose file not found — skipping container cleanup"
    fi

    # ---- Firewall (read state before deleting the config dir) --------------
    _u_firewall_cleanup "$install_dir"

    # ---- systemd unit ------------------------------------------------------
    _u_info "Removing systemd service..."
    _u_try rm -f /etc/systemd/system/serverkit.service
    _u_run systemctl daemon-reload 2>/dev/null || true
    _u_ok "Systemd service removed"

    # ---- nginx ServerKit-owned configs -------------------------------------
    _u_info "Removing nginx configuration..."
    _u_try rm -f /etc/nginx/sites-enabled/serverkit.conf
    _u_try rm -f /etc/nginx/sites-available/serverkit.conf
    _u_try rm -f /etc/nginx/sites-available/serverkit-insecure.conf
    _u_try rm -f /etc/nginx/sites-available/example.conf.template
    _u_try rm -f /etc/nginx/conf.d/serverkit-tls.conf
    _u_try rm -rf /etc/nginx/serverkit-conf.d
    if systemctl is-active --quiet nginx 2>/dev/null; then
        _u_run systemctl reload nginx 2>/dev/null || true
    fi
    _u_ok "Nginx config removed"

    # ---- apt lock-wait drop-in (only the installer creates this) -----------
    _u_try rm -f /etc/apt/apt.conf.d/99-serverkit-lock-wait.conf

    # ---- CLI symlink -------------------------------------------------------
    _u_try rm -f /usr/local/bin/serverkit

    # ---- Code: install dir + blue/green slots + backup snapshot ------------
    _u_info "Removing installation files..."
    _u_try rm -rf "$install_dir"
    _u_try rm -rf "${install_dir}-a"
    _u_try rm -rf "${install_dir}-b"
    _u_try rm -rf "${install_dir}.backup"
    _u_ok "Installation directory removed"

    # ---- Logs (always transient) -------------------------------------------
    _u_try rm -rf "$log_dir"

    # ---- Data tiers --------------------------------------------------------
    if [ "$purge" = "1" ]; then
        _u_info "Purging data (--purge)..."
        _u_try rm -rf "$data_dir" "$apps_dir" "$backup_dir" "$config_dir"
        _u_ok "Data directories removed"
    else
        # Config dir holds install-state.json + ssl-mode + templates. Removed by
        # default, preserved with --keep-data.
        if [ "$keep_data" = "1" ]; then
            _u_info "Preserving data (--keep-data): $data_dir, $config_dir, $backup_dir"
        else
            _u_try rm -rf "$config_dir"
            _u_info "Preserved user data: $data_dir, $apps_dir, $backup_dir"
        fi
    fi

    # ---- Telemetry (best-effort; bounded so an air-gapped box never hangs,
    # ---- and a --dry-run never phones home) ---------------------------------
    if [ "${SERVERKIT_UNINSTALL_DRY_RUN:-0}" != "1" ]; then
        curl -s --max-time 5 "https://serverkit.ai/track/uninstall" >/dev/null 2>&1 || true
    fi
}

# Snapshot the live SQLite DB into the (preserved) backup dir so a default
# uninstall never silently destroys the database with the install tree.
_u_snapshot_db() {
    local install_dir="$1" backup_dir="$2"
    local stamp db_src dest
    stamp="$(date +%Y%m%d-%H%M%S)"
    dest="$backup_dir/serverkit-pre-uninstall-$stamp.db"

    # Hybrid: DB sits in the install tree.
    db_src="$install_dir/backend/instance/serverkit.db"
    if [ -f "$db_src" ]; then
        _u_try mkdir -p "$backup_dir"
        if _u_run cp "$db_src" "$dest" 2>/dev/null; then
            _u_ok "Database snapshot saved to $dest"
            return 0
        fi
    fi

    # All-Docker: DB lives in the backend container's volume.
    if command -v docker >/dev/null 2>&1 && \
       docker ps -a --format '{{.Names}}' 2>/dev/null | grep -qx 'serverkit-backend'; then
        _u_try mkdir -p "$backup_dir"
        if _u_run docker cp serverkit-backend:/app/instance/serverkit.db "$dest" 2>/dev/null; then
            _u_ok "Database snapshot saved to $dest"
            return 0
        fi
    fi

    _u_warn "No live SQLite database found to snapshot — skipping"
}
