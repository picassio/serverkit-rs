#!/usr/bin/env bash
#
# Smoke-test the shipped nginx site configs with the real `nginx -t` parser.
#
# The panel's frontend is served straight from these configs (static SPA at
# `location /`, /api + /socket.io proxied to the host backend). A broken
# directive — a bad root/try_files, a stray brace, a duplicate default_server —
# would 502 the whole panel on a real box. `nginx -t` catches that whole class
# before it ships; this wraps it so it runs the same in CI and on any box that
# has nginx.
#
# It stages each shipped vhost (secure + insecure) one at a time against a real
# nginx, recreating the absolute paths the configs reference (the SPA bundle, the
# include dirs) and a throwaway self-signed cert for the TLS vhost.
#
# Skips cleanly (exit 0) where nginx isn't installed or root isn't reachable —
# so it's safe to run on a dev laptop; CI installs nginx so it actually runs.
#
# Run:  bash scripts/test/test_nginx_conf.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/../.." && pwd)"
SITES="$REPO/nginx/sites-available"

PASS=0; FAIL=0; SKIP=0
ok()   { PASS=$((PASS + 1)); printf '  \033[32m✔\033[0m %s\n' "$1"; }
bad()  { FAIL=$((FAIL + 1)); printf '  \033[31m✘\033[0m %s\n' "$1"; }
skip() { SKIP=$((SKIP + 1)); printf '  \033[33m∼\033[0m %s (skipped)\n' "$1"; }

summary() { printf '\n%d passed, %d failed, %d skipped\n\n' "$PASS" "$FAIL" "$SKIP"; }

printf '\nnginx config smoke test\n\n'

if ! command -v nginx >/dev/null 2>&1; then
    skip "nginx not installed — install nginx to run this smoke test"
    summary; exit 0
fi

# nginx -t reads the system prefix; it needs root. Use sudo when we are not root,
# and skip (don't fail) if neither is available — keeps dev laptops green.
SUDO=()
if [ "$(id -u)" -ne 0 ]; then
    if command -v sudo >/dev/null 2>&1; then
        SUDO=(sudo)
    else
        skip "nginx -t needs root and sudo is unavailable"
        summary; exit 0
    fi
fi

if ! command -v openssl >/dev/null 2>&1; then
    skip "openssl unavailable — cannot mint a test cert for the TLS vhost"
    summary; exit 0
fi

WORK="$(mktemp -d)"
CERT_DIR="$WORK/cert"; mkdir -p "$CERT_DIR"
cleanup() {
    "${SUDO[@]}" rm -f /etc/nginx/sites-enabled/serverkit-smoke.conf 2>/dev/null || true
    rm -rf "$WORK"
}
trap cleanup EXIT

# Recreate the absolute paths the vhosts reference: a stand-in SPA bundle so
# `root .../frontend/dist` resolves, the per-app + conf.d include dirs (empty is
# fine — nginx tolerates a glob that matches nothing), and certbot's webroot.
"${SUDO[@]}" mkdir -p /etc/nginx/serverkit-locations /etc/nginx/serverkit-conf.d \
    /var/www/certbot /opt/serverkit/frontend/dist /etc/nginx/sites-available \
    /etc/nginx/sites-enabled
printf '<!doctype html><title>serverkit</title>\n' \
    | "${SUDO[@]}" tee /opt/serverkit/frontend/dist/index.html >/dev/null

# A throwaway self-signed cert for the secure vhost's ssl_certificate paths.
openssl req -x509 -newkey rsa:2048 -nodes -days 1 \
    -keyout "$CERT_DIR/privkey.pem" -out "$CERT_DIR/fullchain.pem" \
    -subj "/CN=smoke.test" >/dev/null 2>&1
chmod 644 "$CERT_DIR"/*.pem

# The shipped vhosts declare `listen ... default_server`; the distro default site
# does too, which would trip a duplicate-default_server error. Move it aside for
# the duration of the test. It must land OUTSIDE sites-enabled/ — nginx.conf
# includes `sites-enabled/*` (bare glob), so a `.smoke-bak` suffix left in that
# dir would still be loaded and still collide.
DEFAULT_SITE=/etc/nginx/sites-enabled/default
DEFAULT_BAK=/etc/nginx/default.smoke-bak
DEFAULT_MOVED=0
if [ -e "$DEFAULT_SITE" ]; then
    "${SUDO[@]}" mv "$DEFAULT_SITE" "$DEFAULT_BAK" && DEFAULT_MOVED=1
fi
restore_default() {
    [ "$DEFAULT_MOVED" = "1" ] && "${SUDO[@]}" mv "$DEFAULT_BAK" "$DEFAULT_SITE" 2>/dev/null || true
}
trap 'restore_default; cleanup' EXIT

# Ensure nginx.conf actually includes sites-enabled (Debian/Ubuntu does out of
# the box; a minimal nginx.conf might not).
if ! "${SUDO[@]}" grep -q 'sites-enabled' /etc/nginx/nginx.conf 2>/dev/null; then
    "${SUDO[@]}" sed -i '/http {/a \    include /etc/nginx/sites-enabled/*;' /etc/nginx/nginx.conf
fi

test_conf() {
    local label="$1" src="$2" staged
    staged="$WORK/$(basename "$src")"
    cp "$src" "$staged"
    # Point the secure vhost's placeholder cert path at the self-signed cert.
    sed -i "s|/etc/letsencrypt/live/YOUR_DOMAIN/|$CERT_DIR/|g" "$staged"
    "${SUDO[@]}" cp "$staged" /etc/nginx/sites-available/serverkit-smoke.conf
    "${SUDO[@]}" ln -sf /etc/nginx/sites-available/serverkit-smoke.conf \
        /etc/nginx/sites-enabled/serverkit-smoke.conf
    if "${SUDO[@]}" nginx -t >/dev/null 2>&1; then
        ok "$label passes nginx -t"
    else
        bad "$label failed nginx -t:"
        "${SUDO[@]}" nginx -t 2>&1 | sed 's/^/      /'
    fi
    "${SUDO[@]}" rm -f /etc/nginx/sites-enabled/serverkit-smoke.conf \
        /etc/nginx/sites-available/serverkit-smoke.conf
}

test_conf "serverkit-insecure.conf (HTTP)"  "$SITES/serverkit-insecure.conf"
test_conf "serverkit.conf (HTTPS)"          "$SITES/serverkit.conf"

summary
[ "$FAIL" -eq 0 ]
