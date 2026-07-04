#!/usr/bin/env bash
#
# Unit tests for the `serverkit` CLI (repo root).
#
# The CLI is source-able: when sourced it defines every function and returns
# *before* the command dispatch (the BASH_SOURCE guard), exactly like
# scripts/update.sh. Each unit under test runs in a subshell that re-arms
# `set -Eeuo pipefail`, so a "benign non-zero aborts the script" regression
# (the class the shell-surface audit found: L5/L6/L7/L9/L12) fails here as an
# assertion instead of on a real box.
#
# External tools (systemctl/docker/nginx/journalctl/curl/...) are PATH-stubbed,
# so this runs deterministically on any host, including Windows/Git-Bash.
#
# Run:  bash scripts/test/test_cli.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLI="$SCRIPT_DIR/../../serverkit"

# Shared stub factories.
# shellcheck source=stubs.sh
source "$SCRIPT_DIR/stubs.sh"

PASS=0
FAIL=0
SKIP=0
ok()   { PASS=$((PASS + 1)); printf '  \033[32m✔\033[0m %s\n' "$1"; }
bad()  { FAIL=$((FAIL + 1)); printf '  \033[31m✘\033[0m %s\n' "$1"; }
skip() { SKIP=$((SKIP + 1)); printf '  \033[33m∼\033[0m %s (skipped)\n' "$1"; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK" 2>/dev/null || true' EXIT   # teardown is best-effort

# --------------------------------------------------------------------------
# Global stubs: harmless defaults for every external tool the CLI may shell
# out to, so nothing ever touches the host's systemd/docker/nginx. Per-test
# stub dirs are PATH-prepended when a test needs a failing/recording variant.
# --------------------------------------------------------------------------
STUB_BIN="$WORK/bin"
mkdir -p "$STUB_BIN"
make_stub_ok "$STUB_BIN" systemctl docker nginx journalctl curl sleep ss
# Healthy system probes for doctor (free does not exist on Git-Bash/macOS).
cat > "$STUB_BIN/df" <<'EOF'
#!/usr/bin/env bash
printf 'Filesystem 1K-blocks Used Available Use%% Mounted\n'
printf '/dev/sda1 1000000 420000 580000 42%% /\n'
EOF
cat > "$STUB_BIN/free" <<'EOF'
#!/usr/bin/env bash
printf '       total  used  free  shared  buff/cache  available\n'
printf 'Mem:    2000   400   600      10        1000       1500\n'
printf 'Swap:   1024     0  1024\n'
EOF
chmod +x "$STUB_BIN"/*
export PATH="$STUB_BIN:$PATH"

# --------------------------------------------------------------------------
# Source the CLI (functions only — the BASH_SOURCE guard skips the dispatch).
# Point the install/nginx dirs into the sandbox so nothing touches the host.
# --------------------------------------------------------------------------
export SERVERKIT_DIR="$WORK/opt/serverkit"
export SERVERKIT_NGINX_DIR="$WORK/etc/nginx"
# shellcheck disable=SC1090
source "$CLI"
set +e +u   # hand control back to the harness; tests re-arm per subshell

printf '\nserverkit CLI unit tests\n\n'

# --------------------------------------------------------------------------
# T1 — L5: `serverkit update --branch` with NO value used to die on `shift 2`
# under set -e with zero output. It must fail loudly instead.
# --------------------------------------------------------------------------
out="$( set -Eeuo pipefail; cmd_update --branch 2>&1 )"
rc=$?
if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -q -- '--branch requires'; then
    ok "update --branch with no value fails loudly (rc=$rc, message printed)"
else
    bad "update --branch with no value: rc=$rc out=[$out] — silent or passing"
fi

# T1b — a valid --branch still forwards to the canonical updater unchanged.
t="$WORK/t1b"; mkdir -p "$t/scripts"
printf '#!/usr/bin/env bash\nprintf "ARGS:%%s\\n" "$*"\n' > "$t/scripts/update.sh"
chmod +x "$t/scripts/update.sh"
if ! out="$(
    set -Eeuo pipefail
    check_root() { :; }
    check_installed() { :; }
    INSTALL_DIR="$t"; VENV_DIR="$t/venv"
    cmd_update --branch dev --force 2>&1
)"; then
    bad "update --branch dev returned non-zero against a succeeding updater: [$out]"
elif printf '%s' "$out" | grep -q 'ARGS:--branch dev --force'; then
    ok "update --branch <name> still forwards args to scripts/update.sh"
else
    bad "update --branch dev forwarding broken: [$out]"
fi

# --------------------------------------------------------------------------
# T2 — L6: cmd_start must warn (not raw-abort) when systemctl/docker fail
# (partial installs, non-systemd boxes). cmd_stop already had these guards.
# --------------------------------------------------------------------------
t="$WORK/t2"; mkdir -p "$t/bin" "$t/install"
CALL_LOG="$t/calls.log"; : > "$CALL_LOG"
cat > "$t/bin/systemctl" <<EOF
#!/usr/bin/env bash
printf 'systemctl %s\n' "\$*" >> "$CALL_LOG"
exit 1
EOF
cat > "$t/bin/docker" <<EOF
#!/usr/bin/env bash
printf 'docker %s\n' "\$*" >> "$CALL_LOG"
exit 1
EOF
chmod +x "$t/bin"/*
out="$(
    set -Eeuo pipefail
    export PATH="$t/bin:$PATH"
    check_root() { :; }
    INSTALL_DIR="$t/install"
    cmd_start 2>&1
)"
rc=$?
if [ "$rc" -eq 0 ] \
   && printf '%s' "$out" | grep -q 'Could not start backend' \
   && printf '%s' "$out" | grep -q 'Could not start frontend' \
   && grep -q 'docker compose up -d' "$CALL_LOG"; then
    ok "start survives failing systemctl/docker with warnings (no raw abort)"
else
    bad "start aborted or lost a warning: rc=$rc out=[$out]"
fi

# --------------------------------------------------------------------------
# T3 — L6: cmd_restart, same shape.
# --------------------------------------------------------------------------
t="$WORK/t3"; mkdir -p "$t/bin" "$t/install"
CALL_LOG="$t/calls.log"; : > "$CALL_LOG"
cat > "$t/bin/systemctl" <<EOF
#!/usr/bin/env bash
printf 'systemctl %s\n' "\$*" >> "$CALL_LOG"
exit 1
EOF
cat > "$t/bin/docker" <<EOF
#!/usr/bin/env bash
printf 'docker %s\n' "\$*" >> "$CALL_LOG"
exit 1
EOF
chmod +x "$t/bin"/*
out="$(
    set -Eeuo pipefail
    export PATH="$t/bin:$PATH"
    check_root() { :; }
    INSTALL_DIR="$t/install"
    cmd_restart 2>&1
)"
rc=$?
if [ "$rc" -eq 0 ] \
   && printf '%s' "$out" | grep -q 'Could not restart backend' \
   && printf '%s' "$out" | grep -q 'ServerKit restarted' \
   && grep -q 'systemctl restart serverkit' "$CALL_LOG"; then
    ok "restart survives failing systemctl/docker with warnings (no raw abort)"
else
    bad "restart aborted or lost a warning: rc=$rc out=[$out]"
fi

# --------------------------------------------------------------------------
# T4 — L7: `serverkit logs` on a journal-less box must not die before showing
# the Docker logs that would have worked.
# --------------------------------------------------------------------------
t="$WORK/t4"; mkdir -p "$t/bin" "$t/install"
CALL_LOG="$t/calls.log"; : > "$CALL_LOG"
cat > "$t/bin/journalctl" <<EOF
#!/usr/bin/env bash
printf 'journalctl %s\n' "\$*" >> "$CALL_LOG"
exit 1
EOF
cat > "$t/bin/docker" <<EOF
#!/usr/bin/env bash
printf 'docker %s\n' "\$*" >> "$CALL_LOG"
exit 0
EOF
chmod +x "$t/bin"/*
out="$(
    set -Eeuo pipefail
    export PATH="$t/bin:$PATH"
    INSTALL_DIR="$t/install"
    cmd_logs all 2>&1
)"
rc=$?
if [ "$rc" -eq 0 ] \
   && printf '%s' "$out" | grep -q 'Could not read the backend journal' \
   && grep -q 'docker compose logs --tail=50' "$CALL_LOG"; then
    ok "logs falls through a failing journalctl to the Docker-logs half"
else
    bad "logs died before the Docker half: rc=$rc out=[$out]"
fi

# --------------------------------------------------------------------------
# T5 — L9: add-site on a Debian layout (sites-available present) writes the
# vhost, enables it, and reloads.
# --------------------------------------------------------------------------
t="$WORK/t5"; mkdir -p "$t/nginx/sites-available" "$t/nginx/sites-enabled"
out="$(
    set -Eeuo pipefail
    check_root() { :; }
    NGINX_DIR="$t/nginx"
    cmd_add_site app.example.com 3100 2>&1
)"
rc=$?
if [ "$rc" -eq 0 ] \
   && [ -f "$t/nginx/sites-available/app.example.com.conf" ] \
   && [ -e "$t/nginx/sites-enabled/app.example.com.conf" ] \
   && grep -q 'proxy_pass http://127.0.0.1:3100;' "$t/nginx/sites-available/app.example.com.conf" \
   && printf '%s' "$out" | grep -q 'Site added'; then
    ok "add-site (Debian layout) writes, enables, and reports the site"
else
    bad "add-site Debian layout broken: rc=$rc out=[$out]"
fi

# --------------------------------------------------------------------------
# T6 — L9: add-site on a conf.d-only layout (RHEL family) must not assume
# sites-available/sites-enabled exist.
# --------------------------------------------------------------------------
t="$WORK/t6"; mkdir -p "$t/nginx"
out="$(
    set -Eeuo pipefail
    check_root() { :; }
    NGINX_DIR="$t/nginx"
    cmd_add_site rhel.example.com 3200 2>&1
)"
rc=$?
if [ "$rc" -eq 0 ] \
   && [ -f "$t/nginx/conf.d/rhel.example.com.conf" ] \
   && [ ! -d "$t/nginx/sites-enabled" ]; then
    ok "add-site (conf.d layout) writes into conf.d and skips the symlink"
else
    bad "add-site conf.d layout broken: rc=$rc out=[$out]"
fi

# --------------------------------------------------------------------------
# T7 — L9: a stopped nginx / non-systemd box must not raw-abort AFTER the
# site file is written — warn with a reload-manually hint instead.
# --------------------------------------------------------------------------
t="$WORK/t7"; mkdir -p "$t/bin" "$t/nginx/sites-available" "$t/nginx/sites-enabled"
printf '#!/usr/bin/env bash\nexit 1\n' > "$t/bin/systemctl"
cat > "$t/bin/nginx" <<'EOF'
#!/usr/bin/env bash
[ "${1:-}" = "-t" ] && exit 0
exit 1
EOF
chmod +x "$t/bin"/*
out="$(
    set -Eeuo pipefail
    export PATH="$t/bin:$PATH"
    check_root() { :; }
    NGINX_DIR="$t/nginx"
    cmd_add_site down.example.com 3300 2>&1
)"
rc=$?
if [ "$rc" -eq 0 ] \
   && [ -f "$t/nginx/sites-available/down.example.com.conf" ] \
   && printf '%s' "$out" | grep -q 'Could not reload nginx' \
   && printf '%s' "$out" | grep -q 'Site added'; then
    ok "add-site with a stopped nginx warns 'reload manually' instead of aborting"
else
    bad "add-site stopped-nginx path broken: rc=$rc out=[$out]"
fi

# --------------------------------------------------------------------------
# T8 — add-site with a failing `nginx -t` still errors out loudly and
# disables the broken vhost: Debian → drop the symlink; conf.d → park the
# live file as .disabled so it can't poison an unrelated reload.
# --------------------------------------------------------------------------
t="$WORK/t8"; mkdir -p "$t/bin" "$t/nginx/sites-available" "$t/nginx/sites-enabled" "$t/nginx2"
printf '#!/usr/bin/env bash\nexit 1\n' > "$t/bin/nginx"
chmod +x "$t/bin/nginx"
out="$(
    set -Eeuo pipefail
    export PATH="$t/bin:$PATH"
    check_root() { :; }
    NGINX_DIR="$t/nginx"
    cmd_add_site bad.example.com 3400 2>&1
)"
rc=$?
out2="$(
    set -Eeuo pipefail
    export PATH="$t/bin:$PATH"
    check_root() { :; }
    NGINX_DIR="$t/nginx2"
    cmd_add_site bad2.example.com 3500 2>&1
)"
rc2=$?
if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -q 'Nginx config error' \
   && [ ! -e "$t/nginx/sites-enabled/bad.example.com.conf" ] \
   && [ -f "$t/nginx/sites-available/bad.example.com.conf" ] \
   && [ "$rc2" -ne 0 ] \
   && [ ! -f "$t/nginx2/conf.d/bad2.example.com.conf" ] \
   && [ -f "$t/nginx2/conf.d/bad2.example.com.conf.disabled" ]; then
    ok "add-site on nginx -t failure disables the vhost in both layouts"
else
    bad "add-site nginx -t failure handling broken: rc=$rc/$rc2 out=[$out] out2=[$out2]"
fi

# --------------------------------------------------------------------------
# T9 — L4-CLI: on a remnants-only box (shared uninstall lib gone), the CLI
# must print exact recovery instructions instead of a bare refusal —
# the local standalone uninstall.sh when it exists, curl|bash otherwise.
# --------------------------------------------------------------------------
t="$WORK/t9"; mkdir -p "$t/install"
: > "$t/install/uninstall.sh"
out="$(
    set -Eeuo pipefail
    check_root() { :; }
    INSTALL_DIR="$t/install"
    cmd_uninstall --yes 2>&1
)"
rc=$?
if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -qF "sudo bash $t/install/uninstall.sh"; then
    ok "uninstall (lib missing, local uninstall.sh present) points at the local script"
else
    bad "uninstall recovery hint (local) missing: rc=$rc out=[$out]"
fi
rm -f "$t/install/uninstall.sh"
out="$(
    set -Eeuo pipefail
    check_root() { :; }
    INSTALL_DIR="$t/install"
    cmd_uninstall --yes 2>&1
)"
rc=$?
if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -q 'curl -fsSL' \
   && printf '%s' "$out" | grep -q 'uninstall.sh | sudo bash'; then
    ok "uninstall (nothing left locally) points at the curl | sudo bash fallback"
else
    bad "uninstall recovery hint (curl) missing: rc=$rc out=[$out]"
fi

# --------------------------------------------------------------------------
# T10 — L12: compare_versions must treat unparsable versions as "update
# available" (rc 2), never as a silent "up to date"; numeric compares intact.
# --------------------------------------------------------------------------
cv() { ( set -Eeuo pipefail; compare_versions "$1" "$2" >/dev/null 2>&1 ); printf '%s' "$?"; }
if [ "$(cv unknown 1.7.7)" = "2" ] && [ "$(cv 1.7.7 '<html>')" = "2" ] \
   && [ "$(cv 1.7.6 1.7.7)" = "2" ] && [ "$(cv 2.0.0 1.9.9)" = "1" ] \
   && [ "$(cv 1.7.7 1.7.7)" = "0" ] && [ "$(cv 1.8 1.8.1)" = "2" ]; then
    ok "compare_versions: unparsable → 'older' (update available); numeric compare intact"
else
    bad "compare_versions matrix wrong: unknown=$(cv unknown 1.7.7) html=$(cv 1.7.7 '<html>') lt=$(cv 1.7.6 1.7.7) gt=$(cv 2.0.0 1.9.9) eq=$(cv 1.7.7 1.7.7) xy=$(cv 1.8 1.8.1)"
fi

# T10b — end-to-end: a box with a missing/corrupt VERSION file must report
# "Update available", not "latest version".
t="$WORK/t10b"; mkdir -p "$t/bin"
printf '#!/usr/bin/env bash\nprintf "9.9.9"\n' > "$t/bin/curl"
chmod +x "$t/bin/curl"
out="$(
    set -Eeuo pipefail
    export PATH="$t/bin:$PATH"
    VERSION_FILE="$t/no-version-file"
    cmd_check_update 2>&1
)"
rc=$?
if [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q 'Update available'; then
    ok "check-update with an unreadable local VERSION reports 'Update available'"
else
    bad "check-update unknown-local path wrong: rc=$rc out=[$out]"
fi

# --------------------------------------------------------------------------
# T11 — L12: doctor must exit non-zero when checks fail (it always exited 0)
# and still run every section to the end despite the failures.
# --------------------------------------------------------------------------
t="$WORK/t11"; mkdir -p "$t/bin" "$t/install" "$t/nginx"
for c in systemctl docker nginx curl; do
    printf '#!/usr/bin/env bash\nexit 1\n' > "$t/bin/$c"
done
chmod +x "$t/bin"/*
out="$(
    set -Eeuo pipefail
    export PATH="$t/bin:$PATH"
    INSTALL_DIR="$t/install"
    NGINX_DIR="$t/nginx"
    cmd_doctor 2>&1
)"
rc=$?
if [ "$rc" -ne 0 ] \
   && printf '%s' "$out" | grep -q 'Found .* issue' \
   && printf '%s' "$out" | grep -q 'Connectivity'; then
    ok "doctor exits non-zero on a broken box and still runs every section"
else
    bad "doctor gate broken: rc=$rc (expected non-zero) or it aborted early"
fi

# --------------------------------------------------------------------------
# T12 — doctor exits 0 on a fully healthy box, and the portable port
# extraction (sed, not GNU grep -oP) still matches ports correctly.
# --------------------------------------------------------------------------
t="$WORK/t12"
mkdir -p "$t/bin" "$t/install/backend/instance" \
         "$t/nginx/sites-available" "$t/nginx/sites-enabled" "$t/nginx/serverkit-locations"
printf 'x\n' > "$t/install/backend/instance/serverkit.db"
cat > "$t/nginx/sites-available/serverkit.conf" <<'EOF'
server {
    listen 80;
    location / {
        proxy_pass http://127.0.0.1:8080;
    }
    location /api {
        proxy_pass http://127.0.0.1:5000;
    }
}
EOF
cat > "$t/bin/docker" <<'EOF'
#!/usr/bin/env bash
case "${1:-}" in
    ps)   printf 'serverkit-frontend\n' ;;
    port) printf '0.0.0.0:8080\n' ;;
esac
exit 0
EOF
cat > "$t/bin/ss" <<'EOF'
#!/usr/bin/env bash
printf 'LISTEN 0 128 127.0.0.1:5000 users:(("gunicorn",pid=1,fd=1))\n'
exit 0
EOF
chmod +x "$t/bin"/*
out="$(
    set -Eeuo pipefail
    export PATH="$t/bin:$PATH"
    INSTALL_DIR="$t/install"
    NGINX_DIR="$t/nginx"
    cmd_doctor 2>&1
)"
rc=$?
if [ "$rc" -eq 0 ] \
   && printf '%s' "$out" | grep -q 'All checks passed' \
   && printf '%s' "$out" | grep -q 'matches frontend container (8080)' \
   && printf '%s' "$out" | grep -q '(5000) has backend listening'; then
    ok "doctor exits 0 when healthy; portable port extraction matches 8080/5000"
else
    bad "doctor healthy path broken: rc=$rc out=[$out]"
fi

# --------------------------------------------------------------------------
# T13 — L12: list-sites port extraction without GNU grep -oP; conf.d layouts
# are listed too.
# --------------------------------------------------------------------------
t="$WORK/t13"; mkdir -p "$t/nginx/sites-enabled" "$t/rhel/conf.d"
cat > "$t/nginx/sites-enabled/proxied.conf" <<'EOF'
server {
    server_name proxied.example.com;
    location / {
        proxy_pass http://127.0.0.1:4100;
    }
}
EOF
cat > "$t/nginx/sites-enabled/static-site.conf" <<'EOF'
server {
    server_name static.example.com;
    root /var/www/static;
}
EOF
cat > "$t/rhel/conf.d/confd.conf" <<'EOF'
server {
    server_name confd.example.com;
    location / {
        proxy_pass http://127.0.0.1:4200;
    }
}
EOF
out="$( set -Eeuo pipefail; NGINX_DIR="$t/nginx"; cmd_list_sites 2>&1 )"
rc=$?
out2="$( set -Eeuo pipefail; NGINX_DIR="$t/rhel"; cmd_list_sites 2>&1 )"
rc2=$?
if [ "$rc" -eq 0 ] && [ "$rc2" -eq 0 ] \
   && printf '%s' "$out" | grep 'proxied.example.com' | grep -q 'port 4100' \
   && printf '%s' "$out" | grep 'static.example.com' | grep -q 'port static' \
   && printf '%s' "$out2" | grep 'confd.example.com' | grep -q 'port 4200'; then
    ok "list-sites extracts ports portably and lists conf.d layouts"
else
    bad "list-sites broken: rc=$rc/$rc2 out=[$out] out2=[$out2]"
fi

# --------------------------------------------------------------------------
# T14 — L12: `read -p` at EOF (no tty) must not abort under set -e; the empty
# answer takes the safe "No" path. Then a piped "y" actually removes the site
# and a failing reload only warns.
# --------------------------------------------------------------------------
t="$WORK/t14"; mkdir -p "$t/bin" "$t/nginx/sites-available" "$t/nginx/sites-enabled"
printf 'server { server_name gone.example.com; }\n' > "$t/nginx/sites-available/gone.example.com.conf"
: > "$t/nginx/sites-enabled/gone.example.com.conf"
printf '#!/usr/bin/env bash\nexit 1\n' > "$t/bin/systemctl"
printf '#!/usr/bin/env bash\nexit 1\n' > "$t/bin/nginx"
chmod +x "$t/bin"/*
out="$(
    set -Eeuo pipefail
    check_root() { :; }
    NGINX_DIR="$t/nginx"
    cmd_remove_site gone.example.com </dev/null 2>&1
)"
rc=$?
if [ "$rc" -eq 0 ] && [ -f "$t/nginx/sites-available/gone.example.com.conf" ]; then
    ok "remove-site read at EOF defaults to 'No' (no silent set -e abort)"
else
    bad "remove-site EOF handling broken: rc=$rc out=[$out]"
fi
out="$(
    printf 'y' | {
        set -Eeuo pipefail
        export PATH="$t/bin:$PATH"
        check_root() { :; }
        NGINX_DIR="$t/nginx"
        cmd_remove_site gone.example.com
    } 2>&1
)"
rc=$?
if [ "$rc" -eq 0 ] \
   && [ ! -f "$t/nginx/sites-available/gone.example.com.conf" ] \
   && [ ! -e "$t/nginx/sites-enabled/gone.example.com.conf" ] \
   && printf '%s' "$out" | grep -q 'Site removed' \
   && printf '%s' "$out" | grep -q 'Could not reload nginx'; then
    ok "remove-site with 'y' removes the vhost and only warns on a dead nginx"
else
    bad "remove-site confirm path broken: rc=$rc out=[$out]"
fi

# --------------------------------------------------------------------------
# T15 — regression guards baked into the source (same spirit as
# test_update.sh T13/T26): the audited failure shapes must never creep back.
# --------------------------------------------------------------------------
if grep -qE '^[^#]*grep -oP' "$CLI"; then
    bad "GNU-only 'grep -oP' is back in the CLI (breaks busybox doctor/list-sites)"
else
    ok "CLI contains no GNU-only 'grep -oP' on code lines"
fi
if grep -n 'read -p' "$CLI" | grep -v '|| true' | grep -q .; then
    bad "a 'read -p' without '|| true' crept back (EOF aborts the CLI under set -e)"
else
    ok "every interactive 'read -p' tolerates EOF (|| true)"
fi
if grep -qF '[ "${BASH_SOURCE[0]}" = "${0}" ] || return 0' "$CLI"; then
    ok "CLI keeps the sourceability guard (functions testable without dispatch)"
else
    bad "the BASH_SOURCE sourceability guard is missing from the CLI"
fi

# --------------------------------------------------------------------------
# T16 — executed-mode sanity: the sourceability guard must not change direct
# execution (help/version work, unknown commands still fail loudly).
# --------------------------------------------------------------------------
out="$(bash "$CLI" version 2>&1)"; rc=$?
out_help="$(bash "$CLI" help 2>&1)"; rc_help=$?
out_bogus="$(bash "$CLI" no-such-command 2>&1)"; rc_bogus=$?
if [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q 'ServerKit version' \
   && [ "$rc_help" -eq 0 ] && printf '%s' "$out_help" | grep -q 'Usage: serverkit' \
   && [ "$rc_bogus" -ne 0 ] && printf '%s' "$out_bogus" | grep -q 'Unknown command'; then
    ok "executed mode intact: version/help succeed, unknown command fails loudly"
else
    bad "executed-mode dispatch broken: version=$rc help=$rc_help bogus=$rc_bogus"
fi

# --------------------------------------------------------------------------
printf '\n%d passed, %d failed, %d skipped\n\n' "$PASS" "$FAIL" "$SKIP"
[ "$FAIL" -eq 0 ]
