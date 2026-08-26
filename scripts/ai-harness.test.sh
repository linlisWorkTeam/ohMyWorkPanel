#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

bash -n scripts/ai-harness.sh
scripts/ai-harness.sh commit-message "refactor: organize code by product domain" >/dev/null
if scripts/ai-harness.sh commit-message "unstructured message" >/dev/null 2>&1; then
  echo "expected invalid commit message to fail" >&2
  exit 1
fi
scripts/ai-harness.sh check
echo "AI harness contract: OK"
