#!/usr/bin/env bash
# Extension Host purity: no per-extension platform forks (proxy_<ext> / hotelTab / …).
# Allowlist: extensions.rs, ExtensionPanel.tsx, LivePanel bridge, config/docs/tests.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

FAIL=0

check_forbidden() {
  local label="$1"
  local pattern="$2"
  shift 2
  local hits
  hits="$(rg -n --glob '!**/node_modules/**' --glob '!**/target/**' -e "$pattern" "$@" 2>/dev/null || true)"
  if [[ -n "$hits" ]]; then
    echo "FAIL [$label]: forbidden pattern /$pattern/"
    echo "$hits"
    FAIL=1
  fi
}

# New extension must not get dedicated host code in App / web core.
check_forbidden "no-hotel-fork" \
  'proxy_aihotel|HotelPanel|hotelTab|aiHotelStatus|ai_hotel_status|load_aihotel' \
  src/App.tsx src-tauri/src/web.rs src-tauri/src/scheduler.rs src-tauri/src/commands.rs

# Only generic proxy_extension in web.rs (no proxy_<name> handlers).
if rg -n 'fn[[:space:]]+proxy_[a-zA-Z0-9_]+' src-tauri/src/web.rs 2>/dev/null | rg -v 'fn[[:space:]]+proxy_extension' >/tmp/ohmyworkpanel-purity-proxy.txt 2>/dev/null; then
  if [[ -s /tmp/ohmyworkpanel-purity-proxy.txt ]]; then
    echo "FAIL [no-dedicated-proxy]: web.rs must only define proxy_extension"
    cat /tmp/ohmyworkpanel-purity-proxy.txt
    FAIL=1
  fi
fi
rm -f /tmp/ohmyworkpanel-purity-proxy.txt

# App must not hardcode a single-extension tab helper name.
check_forbidden "no-app-ext-status-helper" \
  'aiHotelStatus|hotelTabEnabled' \
  src/App.tsx

if [[ "$FAIL" -ne 0 ]]; then
  echo "==> extension purity: FAILED"
  exit 1
fi

echo "==> extension purity: OK"
