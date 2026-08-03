#!/usr/bin/env bash
# Promote canary artifacts → production slot (binary + dist only).
# Production SQLite / LinlisWorkPanel group data is NEVER overwritten.
set -euo pipefail
source "$(dirname "$0")/release-layout.sh"

echo "==> promote-canary → prod"
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

systemctl restart linlis-work-panel.service
sleep 1
systemctl is-active linlis-work-panel.service
curl -sS -o /dev/null -w "prod_http=%{http_code}\n" "http://127.0.0.1:${PROD_PORT}/" || true
echo "Promoted. Prod UI/binary updated; data still at ${PROD_DATA}"
echo "Login + LinlisWorkPanel group should remain intact."
