#!/usr/bin/env bash
# 版本 / 文档 SSOT 一致性校验（Codex 建议 #10）。
# 必过项：package.json / src-tauri/Cargo.toml / src-tauri/tauri.conf.json 三处版本号一致。
# 参考项（仅警告，不 fail）：HEAD 最近 tag 与版本号一致；docs/version-pipeline.md 已登记 v<版本>。
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

fail=0
warn=0

get_pkg()  { grep -oP '"version"\s*:\s*"\K[^"]+' package.json | head -1; }
get_cargo(){ grep -oP '^version\s*=\s*"\K[^"]+' src-tauri/Cargo.toml | head -1; }
get_conf() { grep -oP '"version"\s*:\s*"\K[^"]+' src-tauri/tauri.conf.json | head -1; }

PKG=$(get_pkg); CARGO=$(get_cargo); CONF=$(get_conf)
echo "versions: package.json=${PKG:-?} Cargo.toml=${CARGO:-?} tauri.conf.json=${CONF:-?}"

if [[ -z "$PKG" || -z "$CARGO" || -z "$CONF" ]]; then
  echo "FAIL: 某个版本号为空" >&2; fail=1
fi
if [[ "$PKG" != "$CARGO" || "$PKG" != "$CONF" ]]; then
  echo "FAIL: 三方版本号不一致（package.json=${PKG} vs Cargo=${CARGO} vs tauri=${CONF}）" >&2
  fail=1
fi

# 参考：最近 tag
LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || true)
if [[ -n "$LATEST_TAG" ]]; then
  TAG_VERSION="${LATEST_TAG#v}"
  if [[ "$TAG_VERSION" != "$PKG" ]]; then
    echo "WARN: 最新 tag ${LATEST_TAG} 与版本 ${PKG} 不一致（发版时打 tag 即可）" >&2
    warn=1
  else
    echo "ok: tag ${LATEST_TAG} 与版本一致"
  fi
else
  echo "WARN: 无 git tag（CI/发版前请打 tag）" >&2
  warn=1
fi

# 参考：docs/version-pipeline.md 是否登记 v<版本>
if grep -q "v${PKG}" docs/version-pipeline.md 2>/dev/null; then
  echo "ok: docs/version-pipeline.md 已登记 v${PKG}"
else
  echo "WARN: docs/version-pipeline.md 未登记 v${PKG}（SSOT 文档漂移）" >&2
  warn=1
fi

echo "==> check-ssot: $([ $fail -eq 0 ] && echo OK || echo FAILED) (${warn} warnings)"
exit $fail
