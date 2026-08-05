#!/usr/bin/env bash
# Root/admin: create a one-shot, 15-minute approval for promote/freeze.
# Usage: ./scripts/approve-prod-release.sh "linli: promote after canary smoke OK"
set -euo pipefail
source "$(dirname "$0")/release-layout.sh"

REASON="${*:-}"
if [[ -z "${REASON}" ]]; then
  echo "Usage: $0 \"<who>: <why this prod change is approved>\"" >&2
  exit 1
fi

mkdir -p "${RELEASE_ROOT}"
FILE="${RELEASE_ROOT}/PROD_APPROVE"
STAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
umask 077
cat > "${FILE}" <<EOF
approved ${STAMP}
by ${USER:-root}
reason ${REASON}
EOF
chmod 600 "${FILE}"
echo "Wrote one-shot approval: ${FILE}"
echo "Valid for 15 minutes. Next: ./scripts/promote-canary.sh  (or freeze-prod)"
echo "Token is consumed on first successful gate check."
