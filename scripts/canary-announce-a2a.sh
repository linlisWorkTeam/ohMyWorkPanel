#!/usr/bin/env bash
# After deploy-canary: ask canary「灰度测试」群管理员（A2A @mention）在群内推送本次改动点。
# Usage:
#   ./scripts/canary-announce-a2a.sh
#   ./scripts/canary-announce-a2a.sh "自定义改动摘要（可多行）"
# Env: CANARY_PORT (default 8081), CANARY_USER/CANARY_PASS (default root/root),
#      CANARY_GROUP_NAME (default 灰度测试), CANARY_ANNOUNCE_LOG_N (default 8)

set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

CANARY_PORT="${CANARY_PORT:-8081}"
CANARY_USER="${CANARY_USER:-root}"
CANARY_PASS="${CANARY_PASS:-root}"
CANARY_GROUP_NAME="${CANARY_GROUP_NAME:-灰度测试}"
CANARY_ANNOUNCE_LOG_N="${CANARY_ANNOUNCE_LOG_N:-8}"
BASE="http://127.0.0.1:${CANARY_PORT}"

NOTES="${1:-}"
if [[ -z "$NOTES" ]]; then
  NOTES="$(git log -n "$CANARY_ANNOUNCE_LOG_N" --pretty=format:'- %h %s' 2>/dev/null || echo '- （无法读取 git log，请手工补充改动点）')"
fi

SHA="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
WHEN="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

TOKEN="$(curl -sS -X POST "$BASE/api/auth/login" \
  -H 'Content-Type: application/json' \
  -d "{\"username\":\"${CANARY_USER}\",\"password\":\"${CANARY_PASS}\"}" \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["token"])')"

python3 - "$BASE" "$TOKEN" "$CANARY_GROUP_NAME" "$SHA" "$WHEN" "$NOTES" <<'PY'
import json, sys, urllib.request

base, token, group_name, sha, when, notes = sys.argv[1:7]

def req(method, path, body=None):
    data = None if body is None else json.dumps(body).encode()
    r = urllib.request.Request(
        base + path,
        data=data,
        method=method,
        headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(r) as resp:
        return json.load(resp)

groups = req("GET", "/api/groups")
group = next((g for g in groups if g.get("name") == group_name), None)
if not group:
    raise SystemExit(f"canary group not found: {group_name}")

state = req("GET", f"/api/groups/{group['id']}")
members = state.get("members") or []
admin_id = group.get("adminMemberId") or state.get("group", {}).get("adminMemberId")
admin = next((m for m in members if m.get("id") == admin_id), None)
if not admin or admin.get("kind") != "agent":
    # fall back: first active agent
    admin = next((m for m in members if m.get("kind") == "agent" and m.get("isActive")), None)
if not admin:
    raise SystemExit("no admin/agent member to A2A-mention")

owner_id = group.get("ownerMemberId") or state.get("group", {}).get("ownerMemberId")
sender = next((m for m in members if m.get("id") == owner_id), None) or next(
    (m for m in members if m.get("kind") == "user" and m.get("isActive")), None
)
if not sender:
    raise SystemExit("no sender member")

body = (
    f"@{admin['displayName']} 【灰度发布 · A2A 公告任务】\n"
    f"请在本群推送本次灰度改动点（可直接复述下方清单，勿嵌套委派超过 3 层）。\n\n"
    f"## 灰度发布公告\n"
    f"- 槽位: canary :{base.rsplit(':',1)[-1]}\n"
    f"- HEAD: {sha}\n"
    f"- 时间(UTC): {when}\n"
    f"- 群: {group_name}\n\n"
    f"### 改动点\n{notes}\n"
)

result = req(
    "POST",
    "/api/messages",
    {
        "groupId": group["id"],
        "senderMemberId": sender["id"],
        "content": body,
        "mentionMemberIds": [admin["id"]],
    },
)
msg = result.get("message") or result
print(
    json.dumps(
        {
            "ok": True,
            "groupId": group["id"],
            "groupName": group_name,
            "admin": admin["displayName"],
            "messageId": msg.get("id"),
            "runIds": result.get("runIds"),
        },
        ensure_ascii=False,
    )
)
PY
