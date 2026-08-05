#!/usr/bin/env bash
# Production change gate — require a short-lived root approval file.
# Agents must NOT create this file or set the env bypass without an explicit
# human/root instruction in the current user message.
#
# Root/admin one-shot approve (valid 15 minutes):
#   ./scripts/approve-prod-release.sh "linli: promote canary after 8081 OK"
# Then:
#   ./scripts/promote-canary.sh
#   # or: ./scripts/freeze-prod.sh ...
set -euo pipefail

PROD_APPROVAL_FILE="${PROD_APPROVAL_FILE:-/opt/linlis-workpanel/PROD_APPROVE}"
PROD_APPROVAL_MAX_AGE_SEC="${PROD_APPROVAL_MAX_AGE_SEC:-900}"

require_prod_approval() {
  local action="${1:-production change}"
  if [[ "${LINLIS_ALLOW_PROD_WITHOUT_APPROVAL:-}" == "1" ]]; then
    echo "WARNING: LINLIS_ALLOW_PROD_WITHOUT_APPROVAL=1 — skipping approval gate for: ${action}" >&2
    return 0
  fi

  if [[ ! -f "${PROD_APPROVAL_FILE}" ]]; then
    cat >&2 <<EOF
REFUSED: ${action} blocked — missing root approval.

Production promote/freeze requires an explicit admin one-shot token:
  ./scripts/approve-prod-release.sh "<who>: <why>"
Then re-run this script within ${PROD_APPROVAL_MAX_AGE_SEC}s.

Agents: do NOT create ${PROD_APPROVAL_FILE} and do NOT set
LINLIS_ALLOW_PROD_WITHOUT_APPROVAL unless the human root explicitly
ordered this production release in the current message.
EOF
    return 2
  fi

  local now mtime age
  now="$(date +%s)"
  mtime="$(stat -c %Y "${PROD_APPROVAL_FILE}" 2>/dev/null || echo 0)"
  age=$((now - mtime))
  if (( age < 0 || age > PROD_APPROVAL_MAX_AGE_SEC )); then
    echo "REFUSED: ${action} blocked — approval expired (age=${age}s, max=${PROD_APPROVAL_MAX_AGE_SEC}s)." >&2
    echo "Re-approve: ./scripts/approve-prod-release.sh \"<who>: <why>\"" >&2
    rm -f "${PROD_APPROVAL_FILE}" || true
    return 2
  fi

  if ! grep -q '^approved ' "${PROD_APPROVAL_FILE}"; then
    echo "REFUSED: ${action} blocked — invalid approval file format." >&2
    return 2
  fi

  echo "==> prod approval OK (age=${age}s): $(tr '\n' ' ' < "${PROD_APPROVAL_FILE}")"
  # One-shot: consume token so a later agent run cannot reuse it.
  rm -f "${PROD_APPROVAL_FILE}" || true
}
