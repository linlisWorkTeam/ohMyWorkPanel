#!/usr/bin/env bash
# Mandatory quality gate before canary deploy. Fail-closed.
# Break-glass only: OHMYWORKPANEL_SKIP_TEST_GATE=1
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

if [[ "${OHMYWORKPANEL_SKIP_TEST_GATE:-}" == "1" ]]; then
  echo "WARNING: OHMYWORKPANEL_SKIP_TEST_GATE=1 - skipping test gate (break-glass only)" >&2
  exit 0
fi

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
# Keep Node heap modest on 2GB hosts. Never ulimit -v the whole script (Vitest Wasm needs headroom).
export NODE_OPTIONS="${NODE_OPTIONS:---max-old-space-size=768}"

echo "==> test-gate: AI contribution harness"
bash "${ROOT}/scripts/ai-harness.test.sh"

echo "==> test-gate: frontend (vitest)"
pnpm exec vitest run --pool=forks --maxWorkers=1

echo "==> test-gate: rust lib (no gui features)"
# Isolate ulimit so it cannot affect this shell after cargo exits.
bash -c '
  set -euo pipefail
  cd "$1/src-tauri"
  ulimit -v 1800000 2>/dev/null || true
  cargo test --no-default-features --lib
' bash "${ROOT}"

echo "==> test-gate: extension host purity"
bash "${ROOT}/scripts/check-extension-purity.sh"

echo "==> test-gate: OK"
