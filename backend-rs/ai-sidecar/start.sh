#!/usr/bin/env bash
# Self-contained launcher. Installs the vendored SDK on first run, then serves.
set -euo pipefail
cd "$(dirname "$0")"
if [ ! -d node_modules/@earendil-works/pi-coding-agent ]; then
  echo "[sk-ai-sidecar] installing dependencies..."
  npm install --no-audit --no-fund
fi
exec node server.mjs
