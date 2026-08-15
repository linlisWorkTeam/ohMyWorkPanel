#!/usr/bin/env bash
# Enable release drain on a WorkPanel port and wait until running==0 (or timeout).
# Usage: drain-wait.sh <port> [timeout_secs]
# Env: LINLIS_DRAIN_USER LINLIS_DRAIN_PASS (default root/root)
# If /api/ops/drain is missing (pre-smooth-release binary), skip wait and exit 0.
set -euo pipefail

PORT="${1:?port required}"
TIMEOUT="${2:-180}"
USER_NAME="${LINLIS_DRAIN_USER:-root}"
PASS="${LINLIS_DRAIN_PASS:-root}"
BASE="http://127.0.0.1:${PORT}"

echo "==> drain-wait :${PORT} timeout=${TIMEOUT}s"

LOGIN_BODY="$(curl -sS -X POST "${BASE}/api/auth/login" \
  -H 'Content-Type: application/json' \
  -d "{\"username\":\"${USER_NAME}\",\"password\":\"${PASS}\"}" || true)"
TOK="$(python3 -c 'import sys,json; 
try:
 d=json.loads(sys.argv[1]); print(d.get("token") or "")
except Exception:
 print("")' "${LOGIN_BODY}")"
if [[ -z "${TOK}" ]]; then
  echo "WARN: drain-wait: login failed; skipping drain" >&2
  exit 0
fi

CODE="$(curl -sS -o /tmp/linlis-drain-status.json -w '%{http_code}' -X PUT "${BASE}/api/ops/drain" \
  -H "Authorization: Bearer ${TOK}" \
  -H 'Content-Type: application/json' \
  -d '{"enabled":true}' || true)"
if [[ "${CODE}" != "200" ]]; then
  echo "WARN: drain-wait: PUT /api/ops/drain -> HTTP ${CODE} (binary may predate drain); skipping wait" >&2
  exit 0
fi

python3 - <<'PY'
import json
d=json.load(open("/tmp/linlis-drain-status.json"))
print(f"drain enabled={d.get('enabled')} running={d.get('running')} queued={d.get('queued')}")
PY

deadline=$((SECONDS + TIMEOUT))
while (( SECONDS < deadline )); do
  CODE="$(curl -sS -o /tmp/linlis-drain-status.json -w '%{http_code}' "${BASE}/api/ops/drain" -H "Authorization: Bearer ${TOK}" || true)"
  if [[ "${CODE}" != "200" ]]; then
    echo "WARN: drain-wait: GET drain HTTP ${CODE}; stopping wait" >&2
    exit 0
  fi
  running="$(python3 -c 'import json; print(json.load(open("/tmp/linlis-drain-status.json")).get("running",0))')"
  queued="$(python3 -c 'import json; print(json.load(open("/tmp/linlis-drain-status.json")).get("queued",0))')"
  echo "  running=${running} queued=${queued}"
  if [[ "${running}" == "0" ]]; then
    echo "==> drain-wait: no running agents; safe to restart"
    exit 0
  fi
  sleep 2
done

echo "WARN: drain-wait timed out with running>0; proceeding (runs will requeue on restart)" >&2
exit 0
