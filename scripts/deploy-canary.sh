#!/usr/bin/env bash
# Build (optional) + install workspace artifacts into CANARY slot, restart canary service.
# Canary uses a SEPARATE data directory — never writes to production SQLite.
set -euo pipefail
source "$(dirname "$0")/release-layout.sh"

BUILD="${1:-auto}" # auto | skip

echo "==> deploy-canary: slot=${CANARY_SLOT} port=${CANARY_PORT}"
mkdir -p "${CANARY_SLOT}/bin" "${CANARY_SLOT}/dist" "${CANARY_SLOT}/meta" "${CANARY_DATA}"

# Quality gate before any build/install (also runs when BUILD=skip).
# Break-glass: LINLIS_SKIP_TEST_GATE=1
echo "==> deploy-canary: running test gate"
bash "$(dirname "$0")/test-gate.sh"

if [[ "${BUILD}" != "skip" ]]; then
  echo "==> building frontend (low memory)"
  sync; echo 3 > /proc/sys/vm/drop_caches 2>/dev/null || true
  (
    cd "${LINLIS_ROOT}"
    export NODE_OPTIONS="${NODE_OPTIONS:---max-old-space-size=1024}"
    pnpm run build:web
  )
  echo "==> building server (CARGO_BUILD_JOBS=1)"
  sync; echo 3 > /proc/sys/vm/drop_caches 2>/dev/null || true
  (
    cd "${LINLIS_ROOT}/src-tauri"
    CARGO_BUILD_JOBS=1 cargo build --release --no-default-features --bin linlis-work-panel-server
  )
fi

if [[ ! -x "${WORKSPACE_BIN}" || ! -f "${WORKSPACE_DIST}/index.html" ]]; then
  echo "ERROR: workspace artifacts missing; build failed or use without skip" >&2
  exit 1
fi

/bin/cp -f "${WORKSPACE_BIN}" "${CANARY_SLOT}/bin/linlis-work-panel-server"
chmod +x "${CANARY_SLOT}/bin/linlis-work-panel-server"
rm -rf "${CANARY_SLOT}/dist"
/bin/cp -a "${WORKSPACE_DIST}" "${CANARY_SLOT}/dist"

STAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
SHA="$(sha256sum "${CANARY_SLOT}/bin/linlis-work-panel-server" | awk '{print $1}')"
cat > "${CANARY_SLOT}/meta/RELEASE.json" <<EOF
{
  "slot": "canary",
  "deployedAt": "${STAMP}",
  "binarySha256": "${SHA}",
  "source": "workspace",
  "port": ${CANARY_PORT},
  "dataDir": "${CANARY_DATA}"
}
EOF

# Codex shim is embedded in linlis-work-panel-server (sidecar on :18888).
# Stop the legacy standalone unit so WorkPanel can own the port.
/bin/cp -f "${LINLIS_ROOT}/deploy/systemd/linlis-work-panel-canary.service" /etc/systemd/system/linlis-work-panel-canary.service
/bin/cp -f "${LINLIS_ROOT}/deploy/systemd/linlis-work-panel.service" /etc/systemd/system/linlis-work-panel.service
systemctl daemon-reload
systemctl enable linlis-work-panel-canary.service >/dev/null 2>&1 || true
if systemctl list-unit-files linlis-codex-proxy.service >/dev/null 2>&1; then
  echo "==> retiring standalone linlis-codex-proxy.service (now embedded)"
  systemctl disable --now linlis-codex-proxy.service >/dev/null 2>&1 || true
  # Ensure port is free for canary's embedded sidecar
  fuser -k 18888/tcp >/dev/null 2>&1 || true
  sleep 1
fi
systemctl restart linlis-work-panel-canary.service
sleep 2
systemctl is-active linlis-work-panel-canary.service
curl -sS -o /dev/null -w "canary_http=%{http_code}\n" "http://127.0.0.1:${CANARY_PORT}/" || true
curl -sS -o /dev/null -w "codex_proxy_health=%{http_code}\n" "http://127.0.0.1:18888/health" || true
# Show who owns :18888 (should be node child of canary server)
ss -ltnp 2>/dev/null | rg '18888' || true
echo "Canary ready: http://<host>:${CANARY_PORT}/  (data=${CANARY_DATA})"
echo "Production untouched: http://<host>:${PROD_PORT}/  (data=${PROD_DATA})"
