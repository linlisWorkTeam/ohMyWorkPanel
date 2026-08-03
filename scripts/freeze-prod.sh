#!/usr/bin/env bash
# Snapshot artifacts into the PRODUCTION slot.
# Modes:
#   from-running  — copy currently installed binary + live dist (default, safest freeze)
#   from-workspace — copy workspace release build artifacts
# Does NOT touch production SQLite data.
set -euo pipefail
source "$(dirname "$0")/release-layout.sh"

MODE="${1:-from-running}"
RUNNING_BIN="${RUNNING_BIN:-/usr/local/bin/linlis-work-panel-server}"

echo "==> freeze-prod: mode=${MODE} slot=${PROD_SLOT}"
mkdir -p "${PROD_SLOT}/bin" "${PROD_SLOT}/dist" "${PROD_SLOT}/meta"

SRC_BIN=""
SRC_DIST=""
case "${MODE}" in
  from-running)
    SRC_BIN="${RUNNING_BIN}"
    SRC_DIST="${WORKSPACE_DIST}"
    ;;
  from-workspace)
    SRC_BIN="${WORKSPACE_BIN}"
    SRC_DIST="${WORKSPACE_DIST}"
    ;;
  *)
    echo "Usage: $0 [from-running|from-workspace]" >&2
    exit 1
    ;;
esac

if [[ ! -x "${SRC_BIN}" ]]; then
  echo "ERROR: missing binary: ${SRC_BIN}" >&2
  exit 1
fi
if [[ ! -f "${SRC_DIST}/index.html" ]]; then
  echo "ERROR: missing frontend dist: ${SRC_DIST}/index.html" >&2
  exit 1
fi

/bin/cp -f "${SRC_BIN}" "${PROD_SLOT}/bin/linlis-work-panel-server"
chmod +x "${PROD_SLOT}/bin/linlis-work-panel-server"
rm -rf "${PROD_SLOT}/dist"
/bin/cp -a "${SRC_DIST}" "${PROD_SLOT}/dist"

STAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
SHA="$(sha256sum "${PROD_SLOT}/bin/linlis-work-panel-server" | awk '{print $1}')"
cat > "${PROD_SLOT}/meta/RELEASE.json" <<EOF
{
  "slot": "prod",
  "frozenAt": "${STAMP}",
  "binarySha256": "${SHA}",
  "source": "${MODE}",
  "port": ${PROD_PORT},
  "dataDir": "${PROD_DATA}"
}
EOF

echo "Frozen prod binary sha256=${SHA}"
echo "Dist: ${PROD_SLOT}/dist"
echo "Data (unchanged): ${PROD_DATA}"
echo "Point systemd at ${PROD_SLOT} then: systemctl restart linlis-work-panel.service"
