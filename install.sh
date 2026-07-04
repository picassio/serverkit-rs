#!/usr/bin/env bash
#
# ServerKit-RS installer — prepares the machine and installs the panel.
#
#   curl -fsSL https://raw.githubusercontent.com/picassio/serverkit-rs/main/install.sh | sudo bash
#
# Piped like above (no local files), it downloads the latest release (or clones
# the repo if there is no release) and re-runs itself from there. From a release
# tarball or a checkout it uses those files directly.
#
# What it does, on a FRESH Ubuntu/Debian VM:
#   1. installs prerequisites  — docker, node, nginx, php-fpm (+ rust if building)
#   2. tunes the OS for Magento — vm.max_map_count, swappiness, file limits, THP
#   3. installs sk-server + the AI sidecar + templates to /opt/serverkit-rs
#   4. generates secrets, installs a systemd service, and bootstraps the admin
#
# Non-interactive bootstrap:
#   SK_BOOTSTRAP_ADMIN_EMAIL=you@example.com SK_BOOTSTRAP_ADMIN_PASSWORD=secret123 \
#     curl -fsSL .../install.sh | sudo -E bash
#
# Env knobs: SERVERKIT_REPO, INSTALL_DIR, SK_DATA_DIR, PORT, SERVERKIT_RUN_USER,
#            SK_SKIP_PREPARE=1 (skip step 1-2), SK_FORCE_SOURCE=1 (build from source).
#
set -euo pipefail

REPO="${SERVERKIT_REPO:-picassio/serverkit-rs}"
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

[ "$(id -u)" -eq 0 ] || die "please run as root (sudo). Example: curl -fsSL .../install.sh | sudo bash"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" 2>/dev/null && pwd || echo /tmp)"

# ---------------------------------------------------------------------------
# Stage 0 — self-bootstrap: if we have neither the binary nor the source here
# (i.e. we were piped from curl), fetch the project and re-exec from there.
# ---------------------------------------------------------------------------
if [ ! -x "$HERE/sk-server" ] && [ ! -d "$HERE/backend-rs" ]; then
  command -v curl >/dev/null || die "curl is required."
  TMP="$(mktemp -d)"
  if [ "${SK_FORCE_SOURCE:-0}" != "1" ]; then
    log "Fetching latest release of $REPO"
    URL="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
            | grep -oE '"browser_download_url": *"[^"]*linux-x86_64\.tar\.gz"' \
            | head -1 | sed -E 's/.*"(https[^"]*)"$/\1/')"
  fi
  if [ -n "${URL:-}" ]; then
    log "Downloading $URL"
    curl -fsSL -o "$TMP/pack.tar.gz" "$URL"
    tar -C "$TMP" -xzf "$TMP/pack.tar.gz"
    SRC="$(find "$TMP" -maxdepth 1 -type d -name 'serverkit-rs-*' | head -1)"
  else
    log "No release found — cloning source (will build from source)"
    command -v git >/dev/null || { apt-get update -qq && apt-get install -y -qq git; }
    git clone --depth 1 "https://github.com/$REPO.git" "$TMP/src"
    SRC="$TMP/src"
  fi
  [ -d "$SRC" ] || die "could not obtain ServerKit-RS sources"
  exec bash "$SRC/install.sh" "$@"
fi

# Decide install mode now (affects which prerequisites we need).
BUILD_FROM_SOURCE=0
{ [ -x "$HERE/sk-server" ] && [ "${SK_FORCE_SOURCE:-0}" != "1" ]; } || BUILD_FROM_SOURCE=1

# ---------------------------------------------------------------------------
# Stage 1 — prepare the machine (deps + OS tuning). Skip with SK_SKIP_PREPARE=1.
# ---------------------------------------------------------------------------
prepare_system() {
  export DEBIAN_FRONTEND=noninteractive
  log "Installing prerequisites (docker, node, nginx, php)…"
  apt-get update -qq
  apt-get install -y -qq nginx php-fpm php-cli curl jq openssl ca-certificates gnupg unzip lsb-release >/dev/null

  if ! command -v docker >/dev/null; then
    log "Installing Docker…"; curl -fsSL https://get.docker.com | sh >/dev/null 2>&1 || warn "docker install failed"
  fi

  local NODE_MAJOR=0
  command -v node >/dev/null && NODE_MAJOR="$(node -v | sed -E 's/v([0-9]+).*/\1/')"
  if [ "$NODE_MAJOR" -lt 18 ]; then
    log "Installing Node.js 22…"
    curl -fsSL https://deb.nodesource.com/setup_22.x | bash - >/dev/null 2>&1
    apt-get install -y -qq nodejs >/dev/null || warn "node install failed"
  fi

  if [ "$BUILD_FROM_SOURCE" = "1" ] && ! command -v cargo >/dev/null; then
    log "Installing Rust toolchain (for source build)…"
    apt-get install -y -qq build-essential pkg-config libssl-dev >/dev/null
    curl -fsSL https://sh.rustup.rs | sh -s -- -y >/dev/null 2>&1
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env" 2>/dev/null || export PATH="$HOME/.cargo/bin:$PATH"
  fi
  tune_os
}

# OS tuning for Magento/OpenSearch workloads (ported from magento-vm-provisioner
# tune-os.sh). The vm.max_map_count bump is REQUIRED for OpenSearch to start.
tune_os() {
  log "Tuning OS (sysctl, limits, THP)…"
  cat > /etc/sysctl.d/99-serverkit.conf <<'EOF'
# ServerKit-RS — Magento/OpenSearch tuning
fs.file-max = 1000000
vm.max_map_count = 262144
vm.overcommit_memory = 1
vm.swappiness = 10
net.core.somaxconn = 4096
net.core.netdev_max_backlog = 300000
net.ipv4.ip_local_port_range = 10240 65000
fs.inotify.max_user_instances = 8192
fs.inotify.max_user_watches = 524288
EOF
  sysctl --system >/dev/null 2>&1 || true

  cat > /etc/security/limits.d/99-serverkit.conf <<'EOF'
*    soft    nofile    65535
*    hard    nofile    65535
*    soft    nproc     65535
*    hard    nproc     65535
root soft    nofile    65535
root hard    nofile    65535
EOF

  # Disable Transparent Huge Pages (recommended for Redis/DB).
  cat > /etc/systemd/system/disable-thp.service <<'EOF'
[Unit]
Description=Disable Transparent Huge Pages
DefaultDependencies=no
After=sysinit.target local-fs.target
Before=basic.target
[Service]
Type=oneshot
ExecStart=/bin/sh -c 'echo never > /sys/kernel/mm/transparent_hugepage/enabled; echo never > /sys/kernel/mm/transparent_hugepage/defrag'
RemainAfterExit=yes
[Install]
WantedBy=basic.target
EOF
  systemctl daemon-reload
  systemctl enable --now disable-thp.service >/dev/null 2>&1 || true
}

if [ "${SK_SKIP_PREPARE:-0}" != "1" ]; then
  prepare_system
else
  log "SK_SKIP_PREPARE=1 — skipping prerequisites + OS tuning"
fi

command -v node >/dev/null || die "node.js (>=18) is required for the AI sidecar."

gen_hex()    { openssl rand -hex 32 2>/dev/null || head -c32 /dev/urandom | od -An -tx1 | tr -d ' \n'; }
gen_fernet() {
  if command -v python3 >/dev/null; then
    python3 -c "import base64,os;print(base64.urlsafe_b64encode(os.urandom(32)).decode())"
  else
    head -c32 /dev/urandom | base64 | tr '+/' '-_'
  fi
}

# ---------------------------------------------------------------------------
# Stage 2 — install binary + assets
# ---------------------------------------------------------------------------
mkdir -p "$INSTALL_DIR" "$DATA_DIR" "$ENV_DIR"

if [ "$BUILD_FROM_SOURCE" = "0" ]; then
  log "Installing prebuilt binary from release tarball"
  cp "$HERE/sk-server" "$INSTALL_DIR/sk-server"
  cp -r "$HERE/frontend"  "$INSTALL_DIR/"
  cp -r "$HERE/ai-sidecar" "$INSTALL_DIR/"
  cp -r "$HERE/templates"  "$INSTALL_DIR/"
else
  log "Building from source (a few minutes)…"
  command -v cargo >/dev/null || { . "$HOME/.cargo/env" 2>/dev/null || export PATH="$HOME/.cargo/bin:$PATH"; }
  command -v cargo >/dev/null || die "cargo (Rust) is required to build from source."
  ( cd "$HERE/backend-rs" && cargo build --release --bin sk-server )
  ( cd "$HERE/frontend" && npm ci && npm run build )
  cp "$HERE/backend-rs/target/release/sk-server" "$INSTALL_DIR/sk-server"
  mkdir -p "$INSTALL_DIR/frontend"
  cp -r "$HERE/frontend/dist" "$INSTALL_DIR/frontend/dist"
  cp -r "$HERE/backend-rs/ai-sidecar" "$INSTALL_DIR/ai-sidecar"
  rm -rf "$INSTALL_DIR/ai-sidecar/node_modules"
  cp -r "$HERE/backend/templates" "$INSTALL_DIR/templates"
fi
chmod +x "$INSTALL_DIR/sk-server" "$INSTALL_DIR/ai-sidecar/start.sh"

# ---------------------------------------------------------------------------
# Stage 3 — env file (secrets generated once), systemd, admin bootstrap
# ---------------------------------------------------------------------------
if [ ! -f "$ENV_FILE" ]; then
  log "Generating secrets → $ENV_FILE"
  cat > "$ENV_FILE" <<EOF
# ServerKit-RS runtime configuration. Keep private (chmod 600).
PORT=$PORT
DATABASE_URL=sqlite://$DATA_DIR/serverkit.db
SK_DATA_DIR=$DATA_DIR
SK_FRONTEND_DIST=$INSTALL_DIR/frontend/dist
SK_TEMPLATES_DIR=$INSTALL_DIR/templates
SK_SIDECAR_DIR=$INSTALL_DIR/ai-sidecar
SK_MAGENTO_BACKUP_DIR=$DATA_DIR/backups

SECRET_KEY=$(gen_hex)
JWT_SECRET_KEY=$(gen_hex)
SERVERKIT_ENCRYPTION_KEY=$(gen_fernet)

# Optional: Cloudflare token for Let's Encrypt DNS-01
# SK_CF_API_TOKEN=

# Optional non-interactive admin bootstrap (first boot only)
SK_BOOTSTRAP_ADMIN_EMAIL=${SK_BOOTSTRAP_ADMIN_EMAIL:-}
SK_BOOTSTRAP_ADMIN_USERNAME=${SK_BOOTSTRAP_ADMIN_USERNAME:-admin}
SK_BOOTSTRAP_ADMIN_PASSWORD=${SK_BOOTSTRAP_ADMIN_PASSWORD:-}
EOF
  chmod 600 "$ENV_FILE"
else
  log "Keeping existing $ENV_FILE (secrets preserved)"
fi

if ! grep -q '^SK_BOOTSTRAP_ADMIN_PASSWORD=.\+' "$ENV_FILE" && [ -t 0 ]; then
  log "Bootstrap the first admin account (leave blank to create it later in the browser)"
  read -rp "  Admin email: " ADMIN_EMAIL || true
  if [ -n "${ADMIN_EMAIL:-}" ]; then
    read -rsp "  Admin password (>=8 chars): " ADMIN_PASS; echo
    sed -i "s|^SK_BOOTSTRAP_ADMIN_EMAIL=.*|SK_BOOTSTRAP_ADMIN_EMAIL=$ADMIN_EMAIL|" "$ENV_FILE"
    sed -i "s|^SK_BOOTSTRAP_ADMIN_PASSWORD=.*|SK_BOOTSTRAP_ADMIN_PASSWORD=$ADMIN_PASS|" "$ENV_FILE"
  fi
fi

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
LimitNOFILE=65535
[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable "$SERVICE" >/dev/null 2>&1 || true
systemctl restart "$SERVICE"

sleep 2
IP="$(hostname -I | awk '{print $1}')"
log "Done."
echo
echo "  ServerKit-RS is running:  http://$IP:$PORT"
echo "  Service:                  systemctl status $SERVICE"
echo "  Config/secrets:           $ENV_FILE"
if grep -q '^SK_BOOTSTRAP_ADMIN_PASSWORD=.\+' "$ENV_FILE"; then
  echo "  Admin:                    $(grep '^SK_BOOTSTRAP_ADMIN_EMAIL=' "$ENV_FILE" | cut -d= -f2)"
else
  echo "  Next:                     open the URL and register the first admin account."
fi
