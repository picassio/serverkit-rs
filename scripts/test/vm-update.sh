#!/bin/bash
# Runs inside the test VM. Exercises scripts/update.sh end-to-end on a real box:
#   1. Install ServerKit (if not already present); capture version V1.
#   2. Run the LOCAL updater (from /opt/serverkit-src) with --force, so the whole
#      blue/green deploy → migrate → atomic-switch → health-check path runs and
#      we test THIS branch's update.sh (SERVERKIT_NO_SELF_UPDATE=1 keeps it from
#      re-execing into whatever update.sh is on the remote).
#   3. Verify /api/v1/system/health returns 200 after the update.
#   4. Re-run WITHOUT --force and confirm the version-gate reports
#      "Already up to date" (the Phase 2 skip-when-current behavior).
#
# Writes OK/FAIL to /tmp/serverkit-update-status and a full log to
# /var/log/serverkit-test-update.log (both pulled back by the harness).
set -u  # not -e: we capture failures and still emit a report

SRC=/opt/serverkit-src
INSTALL_DIR=/opt/serverkit
LOG=/var/log/serverkit-test-update.log
STATUS=/tmp/serverkit-update-status

log()  { echo "[vm-update] $*" | tee -a "$LOG"; }
fail() { echo "[vm-update] FAIL: $*" | tee -a "$LOG"; echo FAIL > "$STATUS"; exit 1; }

mkdir -p "$(dirname "$LOG")"
: > "$LOG"

health()      { curl -fsS http://127.0.0.1:5000/api/v1/system/health >/dev/null 2>&1; }
wait_health() { local i; for i in $(seq 1 "${1:-60}"); do health && return 0; sleep 2; done; return 1; }
read_version() { tr -d '\r\n ' < "$INSTALL_DIR/VERSION" 2>/dev/null || echo unknown; }

[ -d "$SRC" ] || fail "source dir $SRC missing — transfer broken"

# 1. Ensure ServerKit is installed.
if [ ! -d "$INSTALL_DIR" ]; then
    [ -f "$SRC/install.sh" ] || fail "install.sh not found in $SRC"
    log "Step 1/4: installing ServerKit (clones release, installs deps)..."
    bash "$SRC/install.sh" >> "$LOG" 2>&1 || fail "install.sh failed (rc=$?)"
fi
wait_health 60 || fail "backend not healthy after install"
V1="$(read_version)"
log "Installed version: $V1"

# 2. Forced full update via the LOCAL updater.
log "Step 2/4: forced update with the local updater (target main)..."
SERVERKIT_DIR="$INSTALL_DIR" SERVERKIT_NO_SELF_UPDATE=1 \
    bash "$SRC/scripts/update.sh" --force >> "$LOG" 2>&1 \
    || fail "update.sh --force failed (see log)"

# 3. Health after update.
log "Step 3/4: waiting for health after update..."
wait_health 90 || fail "backend not healthy after update"
V2="$(read_version)"
log "Post-update version: $V2"

# 4. Version-gate: a second run without --force should skip.
log "Step 4/4: re-running without --force (expecting 'Already up to date')..."
OUT="$(SERVERKIT_DIR="$INSTALL_DIR" SERVERKIT_NO_SELF_UPDATE=1 \
        bash "$SRC/scripts/update.sh" 2>&1 | tee -a "$LOG")"
if printf '%s' "$OUT" | grep -qi "Already up to date"; then
    log "Version-gate correctly skipped the redundant update."
else
    # Not fatal: the remote main could have advanced between the two runs. The
    # health check below is the real gate.
    log "WARN: second run did not report 'Already up to date' (remote may have advanced)."
fi
wait_health 90 || fail "backend not healthy after the second update run"

log "Update test passed (V1=$V1 -> V2=$V2)"
echo OK > "$STATUS"
exit 0
