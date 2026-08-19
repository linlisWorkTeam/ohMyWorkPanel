#!/usr/bin/env bash
# Build (optional) + install workspace artifacts into CANARY slot, restart canary service.
# Canary uses a SEPARATE data directory — never writes to production SQLite.
set -euo pipefail
source "$(dirname "$0")/release-layout.sh"

BUILD="${1:-auto}" # auto | skip

echo "==> deploy-canary: slot=${CANARY_SLOT} port=${CANARY_PORT}"
mkdir -p "${CANARY_SLOT}/bin" "${CANARY_SLOT}/dist" "${CANARY_SLOT}/meta" "${CANARY_SLOT}/scripts" "${CANARY_DATA}"

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
# Self-contained slot: ship the Codex shim script next to the binary (开箱即用)。
if [[ -f "${LINLIS_ROOT}/scripts/codex-deepseek-proxy.cjs" ]]; then
  /bin/cp -f "${LINLIS_ROOT}/scripts/codex-deepseek-proxy.cjs" "${CANARY_SLOT}/scripts/codex-deepseek-proxy.cjs"
fi

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

# Canary-only unit sync. Never rewrite / restart production units here.
/bin/cp -f "${LINLIS_ROOT}/deploy/systemd/linlis-work-panel-canary.service" \
  /etc/systemd/system/linlis-work-panel-canary.service
systemctl daemon-reload
systemctl enable linlis-work-panel-canary.service >/dev/null 2>&1 || true
if systemctl list-unit-files linlis-codex-proxy.service >/dev/null 2>&1; then
  echo "==> retiring standalone linlis-codex-proxy.service (now embedded)"
  systemctl disable --now linlis-codex-proxy.service >/dev/null 2>&1 || true
fi
# Smooth restart: drain canary agent runs before stop (timeout → requeue on boot)
if systemctl is-active --quiet linlis-work-panel-canary.service; then
  bash "$(dirname "$0")/lib/drain-wait.sh" "${CANARY_PORT}" "${LINLIS_DRAIN_TIMEOUT:-180}" || true
fi
# Canary uses :18889 (see unit). Do NOT fuser-kill :18888 — that is production's Codex shim.
systemctl restart linlis-work-panel-canary.service
sleep 2
systemctl is-active linlis-work-panel-canary.service
# Fail loud if deploy-canary accidentally stopped production.
if ! systemctl is-active --quiet linlis-work-panel.service; then
  echo "ERROR: production linlis-work-panel.service is not active after canary deploy" >&2
  echo "Canary must never take prod down — investigate before continuing." >&2
  systemctl status linlis-work-panel.service --no-pager -l >&2 || true
  exit 1
fi
curl -sS -o /dev/null -w "canary_http=%{http_code}\n" "http://127.0.0.1:${CANARY_PORT}/" || true
curl -sS -o /dev/null -w "canary_codex_health=%{http_code}\n" "http://127.0.0.1:18889/health" || true
curl -sS -o /dev/null -w "prod_http=%{http_code}\n" "http://127.0.0.1:${PROD_PORT}/" || true
ss -ltnp 2>/dev/null | rg '1888[89]|808[01]' || true
# Frontend shell smoke (catch white-screen / broken hashed assets)
CANARY_JS=$(curl -sS "http://127.0.0.1:${CANARY_PORT}/" | sed -n 's/.*src="\(\/assets\/[^"]*\.js\)".*/\1/p' | head -1 || true)
CANARY_CSS=$(curl -sS "http://127.0.0.1:${CANARY_PORT}/" | sed -n 's/.*href="\(\/assets\/[^"]*\.css\)".*/\1/p' | head -1 || true)
if [[ -n "${CANARY_JS}" ]]; then
  curl -sS -o /dev/null -w "canary_js=%{http_code} path=${CANARY_JS}\n" "http://127.0.0.1:${CANARY_PORT}${CANARY_JS}" || true
fi
if [[ -n "${CANARY_CSS}" ]]; then
  curl -sS -o /dev/null -w "canary_css=%{http_code} path=${CANARY_CSS}\n" "http://127.0.0.1:${CANARY_PORT}${CANARY_CSS}" || true
fi
echo "Canary ready: http://<host>:${CANARY_PORT}/  (data=${CANARY_DATA}, codex=:18889)"
echo "Production untouched: http://<host>:${PROD_PORT}/  (data=${PROD_DATA}, codex=:18888)"
echo "UI checklist: docs/release-checklist.md (§F frontend shell + HTTPS wss)"
echo "NEXT (required): ./scripts/canary-announce-a2a.sh   # A2A @灰度测试管理员 推送本次改动点"
echo "Promote to prod requires: ./scripts/approve-prod-release.sh \"...\" && ./scripts/promote-canary.sh"
