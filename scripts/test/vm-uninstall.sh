#!/bin/bash
# Runs inside the test VM. End-to-end test of the unified uninstaller
# (uninstall.sh + scripts/lib/uninstall.sh), proving the data-handling matrix on
# a real box:
#   1. Install ServerKit and seed sample data (a sentinel under /var/lib + a DB).
#   2. Default uninstall (--yes): the install tree is gone, but user data is
#      preserved — the /var/lib sentinel survives AND a pre-uninstall DB snapshot
#      lands in /var/backups/serverkit.
#   3. Reinstall.
#   4. Purge uninstall (--purge --yes): the data dirs (/var/lib, /etc/serverkit)
#      are removed.
#
# Writes OK/FAIL to /tmp/serverkit-uninstall-status and a full log to
# /var/log/serverkit-test-uninstall.log (both pulled back by the harness).
set -u  # not -e: capture failures and still emit a report

SRC=/opt/serverkit-src
INSTALL_DIR=/opt/serverkit
LOG=/var/log/serverkit-test-uninstall.log
STATUS=/tmp/serverkit-uninstall-status
SENTINEL=/var/lib/serverkit/.vm-uninstall-sentinel

log()  { echo "[vm-uninstall] $*" | tee -a "$LOG"; }
fail() { echo "[vm-uninstall] FAIL: $*" | tee -a "$LOG"; echo FAIL > "$STATUS"; exit 1; }

mkdir -p "$(dirname "$LOG")"
: > "$LOG"

install_sk() {
    [ -f "$SRC/install.sh" ] || fail "install.sh not found in $SRC"
    log "Installing ServerKit (clones release, installs deps)..."
    bash "$SRC/install.sh" >> "$LOG" 2>&1 || fail "install.sh failed (rc=$?)"
    [ -d "$INSTALL_DIR" ] || fail "install.sh did not create $INSTALL_DIR"
}

[ -d "$SRC" ] || fail "source dir $SRC missing — transfer broken"

# 1. Install + seed sample data.
log "Step 1/4: install + seed data"
install_sk
mkdir -p /var/lib/serverkit "$INSTALL_DIR/backend/instance"
echo "keep-me" > "$SENTINEL"
[ -f "$INSTALL_DIR/backend/instance/serverkit.db" ] || echo "sample-db" > "$INSTALL_DIR/backend/instance/serverkit.db"
log "Seeded sentinel + database."

# 2. Default uninstall — data must be preserved.
log "Step 2/4: default uninstall (data preserved)"
bash "$SRC/uninstall.sh" --yes >> "$LOG" 2>&1 || fail "default uninstall failed (rc=$?)"
[ ! -d "$INSTALL_DIR" ] || fail "install dir still present after default uninstall"
[ -f "$SENTINEL" ]      || fail "default uninstall destroyed /var/lib/serverkit data"
if ls /var/backups/serverkit/serverkit-pre-uninstall-*.db >/dev/null 2>&1; then
    log "DB snapshot present in /var/backups/serverkit."
else
    fail "default uninstall did not snapshot the database to backups"
fi
log "Default uninstall preserved user data. OK."

# 3. Reinstall.
log "Step 3/4: reinstall"
install_sk

# 4. Purge uninstall — data must be removed.
log "Step 4/4: purge uninstall (--purge)"
bash "$SRC/uninstall.sh" --purge --yes >> "$LOG" 2>&1 || fail "purge uninstall failed (rc=$?)"
[ ! -d "$INSTALL_DIR" ] || fail "install dir still present after purge"
[ ! -e "$SENTINEL" ]    || fail "--purge did not remove /var/lib/serverkit"
[ ! -d /etc/serverkit ] || fail "--purge did not remove /etc/serverkit"
log "Purge removed all data. OK."

echo OK > "$STATUS"
exit 0
