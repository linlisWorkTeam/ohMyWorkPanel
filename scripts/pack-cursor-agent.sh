#!/usr/bin/env bash
# Emit a Cursor-only agent-config bundle (schema v1) from this machine.
# Never prints authInfo / tokens / emails. Does not copy the cursor-agent binary.
set -euo pipefail
python3 - <<'PY'
import json, os, time, shutil

KEEP = [
    "permissions", "version", "editor", "display", "notifications", "hints",
    "modelSlashCommands", "rewind", "model", "hasChangedDefaultModel", "maxMode",
    "maxModeAutoEnabled", "modelParameters", "selectedModel", "approvalMode",
    "autoAcceptWebSearch", "sandbox", "showSandboxIntro",
]
FORBIDDEN_SUBSTR = ("auth", "token", "secret", "password", "email", "userid", "apikey")

def strip_forbidden(obj):
    if isinstance(obj, dict):
        out = {}
        for k, v in obj.items():
            if any(s in k.lower() for s in FORBIDDEN_SUBSTR):
                continue
            out[k] = strip_forbidden(v)
        return out
    if isinstance(obj, list):
        return [strip_forbidden(x) for x in obj]
    return obj

cli_cfg = None
path = os.path.expanduser("~/.cursor/cli-config.json")
if os.path.isfile(path):
    raw = json.load(open(path, encoding="utf-8"))
    cli_cfg = strip_forbidden({k: raw[k] for k in KEEP if k in raw})

model = None
if isinstance(cli_cfg, dict):
    model = (cli_cfg.get("model") or {}).get("modelId") or (cli_cfg.get("selectedModel") or {}).get("modelId")

agent = shutil.which("agent") or shutil.which("cursor-agent") or "agent"
bundle = {
    "schemaVersion": 1,
    "exportedAt": int(time.time() * 1000),
    "exportedBy": "pack-cursor-agent",
    "source": "ohmyworkpanel/v2.0.0-cursor-pack",
    "codex": {"enabled": False},
    "claude": {"enabled": False},
    "opencode": {"enabled": False},
    "cursor": {
        "enabled": True,
        "executable": "agent",
        "model": model,
        "cliConfig": cli_cfg,
    },
    "files": {},
    "agents": [{
        "adapter": "cursor",
        "displayName": "Cursor Agent",
        "memberId": "seed-member-cursor",
        "model": model,
        "executable": "agent",
    }],
    "autoInstall": ["cursor"],
}
text = json.dumps(bundle, ensure_ascii=False, indent=2)
low = text.lower()
for bad in ("authid", "authcachekey", "authinfo", "wuxiaociwong"):
    if bad in low:
        raise SystemExit(f"refusing to emit bundle: found {bad}")
print(text)
PY
