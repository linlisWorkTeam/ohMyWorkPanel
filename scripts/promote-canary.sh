#!/usr/bin/env bash
# Promote canary artifacts → production slot (binary + dist only).
# Production SQLite / LinlisWorkPanel group data is NEVER overwritten.
# Requires root one-shot approval (see scripts/approve-prod-release.sh).
set -euo pipefail
source "$(dirname "$0")/release-layout.sh"
# shellcheck source=lib/prod-approval.sh
source "$(dirname "$0")/lib/prod-approval.sh"

echo "==> promote-canary → prod"
require_prod_approval "promote-canary (canary → production :${PROD_PORT})"
if [[ ! -x "${CANARY_SLOT}/bin/linlis-work-panel-server" ]]; then
  echo "ERROR: canary binary missing at ${CANARY_SLOT}/bin/" >&2
  exit 1
fi
if [[ ! -f "${CANARY_SLOT}/dist/index.html" ]]; then
  echo "ERROR: canary dist missing" >&2
  exit 1
fi

# Backup previous prod slot (artifacts only)
BAK="${RELEASE_ROOT}/prod-prev-$(date -u +%Y%m%dT%H%M%SZ)"
if [[ -d "${PROD_SLOT}/bin" ]]; then
  mkdir -p "${BAK}"
  /bin/cp -a "${PROD_SLOT}/bin" "${BAK}/bin" 2>/dev/null || true
  /bin/cp -a "${PROD_SLOT}/dist" "${BAK}/dist" 2>/dev/null || true
  /bin/cp -a "${PROD_SLOT}/meta" "${BAK}/meta" 2>/dev/null || true
  echo "Previous prod artifacts backed up to ${BAK}"
fi

mkdir -p "${PROD_SLOT}/bin" "${PROD_SLOT}/dist" "${PROD_SLOT}/meta"
/bin/cp -f "${CANARY_SLOT}/bin/linlis-work-panel-server" "${PROD_SLOT}/bin/linlis-work-panel-server"
chmod +x "${PROD_SLOT}/bin/linlis-work-panel-server"
rm -rf "${PROD_SLOT}/dist"
/bin/cp -a "${CANARY_SLOT}/dist" "${PROD_SLOT}/dist"

STAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
SHA="$(sha256sum "${PROD_SLOT}/bin/linlis-work-panel-server" | awk '{print $1}')"
cat > "${PROD_SLOT}/meta/RELEASE.json" <<EOF
{
  "slot": "prod",
  "promotedAt": "${STAMP}",
  "binarySha256": "${SHA}",
  "source": "canary-promote",
  "port": ${PROD_PORT},
  "dataDir": "${PROD_DATA}",
  "previousBackup": "${BAK:-none}"
}
EOF

# Prefer stop→start over restart: if the client is interrupted mid-restart,
# systemd can cancel the remaining start job and leave prod permanently stopped.
# Trap: any exit/interrupt after we stop MUST attempt start (prevents permanent outage).
PROD_STOPPED=0
ensure_prod_started() {
  if [[ "${PROD_STOPPED}" -eq 1 ]]; then
    echo "==> ensure prod start (trap/cleanup)"
    systemctl start linlis-work-panel.service || true
  fi
}
trap ensure_prod_started EXIT INT TERM

# Smooth promote: drain prod agents before stop
if systemctl is-active --quiet linlis-work-panel.service; then
  bash "$(dirname "$0")/lib/drain-wait.sh" "${PROD_PORT}" "${LINLIS_DRAIN_TIMEOUT:-180}" || true
fi

systemctl stop linlis-work-panel.service || true
PROD_STOPPED=1
sleep 1
systemctl start linlis-work-panel.service
PROD_STOPPED=0
sleep 1
if ! systemctl is-active --quiet linlis-work-panel.service; then
  echo "ERROR: prod failed to start after promote — retrying once" >&2
  systemctl start linlis-work-panel.service || true
  sleep 1
fi
if ! systemctl is-active --quiet linlis-work-panel.service; then
  echo "ERROR: prod failed to start after promote" >&2
  systemctl status linlis-work-panel.service --no-pager -l >&2 || true
  exit 1
fi
trap - EXIT INT TERM
# Ensure auth proxy is up (public edge: nginx → :9090 → :8080).
if [[ -f "${LINLIS_ROOT}/deploy/systemd/linlis-work-panel-proxy.service" ]]; then
  /bin/cp -f "${LINLIS_ROOT}/deploy/systemd/linlis-work-panel-proxy.service" \
    /etc/systemd/system/linlis-work-panel-proxy.service
  systemctl daemon-reload
fi
if systemctl list-unit-files linlis-work-panel-proxy.service >/dev/null 2>&1; then
  systemctl enable linlis-work-panel-proxy.service >/dev/null 2>&1 || true
  systemctl restart linlis-work-panel-proxy.service
  sleep 1
  systemctl is-active linlis-work-panel-proxy.service
fi
if systemctl list-unit-files nginx.service >/dev/null 2>&1; then
  systemctl enable nginx.service >/dev/null 2>&1 || true
  systemctl start nginx.service >/dev/null 2>&1 || true
fi
curl -sS -o /dev/null -w "prod_http=%{http_code}\n" "http://127.0.0.1:${PROD_PORT}/" || true
curl -sS -o /dev/null -w "proxy_http=%{http_code}\n" "http://127.0.0.1:9090/" || true
PROD_JS=$(curl -sS "http://127.0.0.1:${PROD_PORT}/" | sed -n 's/.*src="\(\/assets\/[^"]*\.js\)".*/\1/p' | head -1 || true)
PROD_CSS=$(curl -sS "http://127.0.0.1:${PROD_PORT}/" | sed -n 's/.*href="\(\/assets\/[^"]*\.css\)".*/\1/p' | head -1 || true)
if [[ -n "${PROD_JS}" ]]; then
  curl -sS -o /dev/null -w "prod_js=%{http_code} path=${PROD_JS}\n" "http://127.0.0.1:${PROD_PORT}${PROD_JS}" || true
fi
if [[ -n "${PROD_CSS}" ]]; then
  curl -sS -o /dev/null -w "prod_css=%{http_code} path=${PROD_CSS}\n" "http://127.0.0.1:${PROD_PORT}${PROD_CSS}" || true
fi
echo "Promoted. Prod UI/binary updated; data still at ${PROD_DATA}"
echo "Login + LinlisWorkPanel group should remain intact."
echo "UI checklist: docs/release-checklist.md (§F frontend shell + HTTPS wss)"
