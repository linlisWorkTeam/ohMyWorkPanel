#!/usr/bin/env bash
# Shared paths for prod / canary release slots.
set -euo pipefail

export LINLIS_ROOT="${LINLIS_ROOT:-/AI/LinlisWorkPanel}"
export RELEASE_ROOT="${RELEASE_ROOT:-/opt/linlis-workpanel}"

export PROD_SLOT="${RELEASE_ROOT}/prod"
export CANARY_SLOT="${RELEASE_ROOT}/canary"

export PROD_DATA="${PROD_DATA:-${LINLIS_ROOT}/data}"
export CANARY_DATA="${CANARY_DATA:-${LINLIS_ROOT}/data-canary}"

export PROD_PORT="${PROD_PORT:-8080}"
export CANARY_PORT="${CANARY_PORT:-8081}"

export WORKSPACE_BIN="${LINLIS_ROOT}/src-tauri/target/release/linlis-work-panel-server"
export WORKSPACE_DIST="${LINLIS_ROOT}/dist"
