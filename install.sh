#!/usr/bin/env bash
#
# ServerKit-RS installer.
#
#   curl -fsSL https://raw.githubusercontent.com/picassio/serverkit-rs/main/install.sh | sudo bash
#
# or, from a release tarball / a checkout:
#
#   sudo ./install.sh
#
# Modes:
#   - Release tarball (contains ./sk-server) → installs the prebuilt binary.
#   - Source checkout (contains backend-rs/)  → builds with cargo + npm.
#
# Non-interactive bootstrap (optional):
#   SK_BOOTSTRAP_ADMIN_EMAIL=you@example.com SK_BOOTSTRAP_ADMIN_PASSWORD=secret123 sudo ./install.sh
#
set -euo pipefail

# ---- configuration (override via env) -------------------------------------
INSTALL_DIR="${INSTALL_DIR:-/opt/serverkit-rs}"
DATA_DIR="${SK_DATA_DIR:-/var/lib/serverkit}"
ENV_DIR="${ENV_DIR:-/etc/serverkit}"
ENV_FILE="$ENV_DIR/serverkit.env"
PORT="${PORT:-5000}"
RUN_USER="${SERVERKIT_RUN_USER:-root}"
SERVICE="serverkit"

log()  { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m warn:\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(id -u)" -eq 0 ] || die "please run as root (sudo). It installs a systemd service under $INSTALL_DIR."

HERE="$(cd "$(dirname "$0")" && pwd)"
gen_hex() { openssl rand -hex 32 2>/dev/null || head -c32 /dev/urandom | od -An -tx1 | tr -d ' \n'; }
gen_fernet() {
  if command -v python3 >/dev/null; then
    python3 -c "import base64,os;print(base64.urlsafe_b64encode(os.urandom(32)).decode())"
  else
    head -c32 /dev/urandom | base64 | tr '+/' '-_'
  fi
}

# ---- dependencies ----------------------------------------------------------
command -v node >/dev/null || die "node.js (>=18) is required for the AI sidecar. Install it and re-run."
command -v docker >/dev/null || warn "docker not found — data services (MariaDB/Redis/OpenSearch) run in Docker."
command -v nginx  >/dev/null || warn "nginx not found — needed to serve managed sites/stores."

# ---- obtain the binary + assets -------------------------------------------
mkdir -p "$INSTALL_DIR" "$DATA_DIR" "$ENV_DIR"

if [ -x "$HERE/sk-server" ]; then
  log "Installing prebuilt binary from release tarball"
  cp "$HERE/sk-server" "$INSTALL_DIR/sk-server"
  cp -r "$HERE/frontend" "$INSTALL_DIR/"
  cp -r "$HERE/ai-sidecar" "$INSTALL_DIR/"
  cp -r "$HERE/templates" "$INSTALL_DIR/"
elif [ -d "$HERE/backend-rs" ]; then
  log "Building from source (this can take a few minutes)"
  command -v cargo >/dev/null || die "cargo (Rust) is required to build from source. Install via https://rustup.rs"
  ( cd "$HERE/backend-rs" && cargo build --release --bin sk-server )
  ( cd "$HERE/frontend" && npm ci && npm run build )
  cp "$HERE/backend-rs/target/release/sk-server" "$INSTALL_DIR/sk-server"
  mkdir -p "$INSTALL_DIR/frontend"
  cp -r "$HERE/frontend/dist" "$INSTALL_DIR/frontend/dist"
  cp -r "$HERE/backend-rs/ai-sidecar" "$INSTALL_DIR/ai-sidecar"
  rm -rf "$INSTALL_DIR/ai-sidecar/node_modules"
  cp -r "$HERE/backend/templates" "$INSTALL_DIR/templates"
else
  die "run this from a release tarball or the repository root."
fi
chmod +x "$INSTALL_DIR/sk-server" "$INSTALL_DIR/ai-sidecar/start.sh"

# ---- env file (secrets generated once, preserved on re-install) -----------
if [ ! -f "$ENV_FILE" ]; then
  log "Generating secrets → $ENV_FILE"
  cat > "$ENV_FILE" <<EOF
# ServerKit-RS runtime configuration. Keep this file private (chmod 600).
PORT=$PORT
DATABASE_URL=sqlite://$DATA_DIR/serverkit.db
SK_DATA_DIR=$DATA_DIR
SK_FRONTEND_DIST=$INSTALL_DIR/frontend/dist
SK_TEMPLATES_DIR=$INSTALL_DIR/templates
SK_SIDECAR_DIR=$INSTALL_DIR/ai-sidecar
SK_MAGENTO_BACKUP_DIR=$DATA_DIR/backups

# Secrets (auto-generated — do not share)
SECRET_KEY=$(gen_hex)
JWT_SECRET_KEY=$(gen_hex)
SERVERKIT_ENCRYPTION_KEY=$(gen_fernet)

# Optional: Cloudflare token for Let's Encrypt DNS-01
# SK_CF_API_TOKEN=

# Optional non-interactive admin bootstrap (created on first boot only)
SK_BOOTSTRAP_ADMIN_EMAIL=${SK_BOOTSTRAP_ADMIN_EMAIL:-}
SK_BOOTSTRAP_ADMIN_USERNAME=${SK_BOOTSTRAP_ADMIN_USERNAME:-admin}
SK_BOOTSTRAP_ADMIN_PASSWORD=${SK_BOOTSTRAP_ADMIN_PASSWORD:-}
EOF
  chmod 600 "$ENV_FILE"
else
  log "Keeping existing $ENV_FILE (secrets preserved)"
fi

# ---- optional interactive bootstrap ---------------------------------------
if ! grep -q '^SK_BOOTSTRAP_ADMIN_PASSWORD=.\+' "$ENV_FILE" && [ -t 0 ]; then
  log "Bootstrap the first admin account (leave blank to create it later in the browser)"
  read -rp "  Admin email: " ADMIN_EMAIL || true
  if [ -n "${ADMIN_EMAIL:-}" ]; then
    read -rsp "  Admin password (>=8 chars): " ADMIN_PASS; echo
    sed -i "s|^SK_BOOTSTRAP_ADMIN_EMAIL=.*|SK_BOOTSTRAP_ADMIN_EMAIL=$ADMIN_EMAIL|" "$ENV_FILE"
    sed -i "s|^SK_BOOTSTRAP_ADMIN_PASSWORD=.*|SK_BOOTSTRAP_ADMIN_PASSWORD=$ADMIN_PASS|" "$ENV_FILE"
  fi
fi

# ---- systemd service -------------------------------------------------------
log "Installing systemd service ($SERVICE)"
cat > "/etc/systemd/system/$SERVICE.service" <<EOF
[Unit]
Description=ServerKit-RS control panel
After=network.target docker.service

[Service]
Type=simple
User=$RUN_USER
WorkingDirectory=$INSTALL_DIR
EnvironmentFile=$ENV_FILE
ExecStart=$INSTALL_DIR/sk-server
Restart=on-failure
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable "$SERVICE" >/dev/null 2>&1 || true
systemctl restart "$SERVICE"

sleep 2
log "Done."
echo
echo "  ServerKit-RS is running:  http://$(hostname -I | awk '{print $1}'):$PORT"
echo "  Service:                  systemctl status $SERVICE"
echo "  Config/secrets:           $ENV_FILE"
if grep -q '^SK_BOOTSTRAP_ADMIN_PASSWORD=.\+' "$ENV_FILE"; then
  echo "  Admin:                    $(grep '^SK_BOOTSTRAP_ADMIN_EMAIL=' "$ENV_FILE" | cut -d= -f2)"
else
  echo "  Next:                     open the URL and register the first admin account."
fi
