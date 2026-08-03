#!/usr/bin/env bash
# Mandatory quality gate before canary deploy. Fail-closed.
# Break-glass only: LINLIS_SKIP_TEST_GATE=1
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

if [[ "${LINLIS_SKIP_TEST_GATE:-}" == "1" ]]; then
  echo "WARNING: LINLIS_SKIP_TEST_GATE=1 — skipping test gate (break-glass only)" >&2
  exit 0
fi

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
export NODE_OPTIONS="${NODE_OPTIONS:---max-old-space-size=1024}"

echo "==> test-gate: frontend (vitest)"
pnpm test

echo "==> test-gate: rust lib (no gui features)"
(
  cd "${ROOT}/src-tauri"
  # Cap rustc/test VM when supported; Node/Vitest runs without this (Wasm needs headroom).
  ulimit -v 1800000 2>/dev/null || true
  cargo test --no-default-features --lib
)

echo "==> test-gate: OK"
