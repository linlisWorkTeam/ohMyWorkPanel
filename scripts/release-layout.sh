#!/usr/bin/env bash
# Shared paths for prod / canary release slots.
set -euo pipefail

export OHMYWORKPANEL_ROOT="${OHMYWORKPANEL_ROOT:-/AI/ohMyWorkPanel}"
export RELEASE_ROOT="${RELEASE_ROOT:-/opt/ohmyworkpanel}"

export PROD_SLOT="${RELEASE_ROOT}/prod"
export CANARY_SLOT="${RELEASE_ROOT}/canary"

export PROD_DATA="${PROD_DATA:-${OHMYWORKPANEL_ROOT}/data}"
export CANARY_DATA="${CANARY_DATA:-${OHMYWORKPANEL_ROOT}/data-canary}"

export PROD_PORT="${PROD_PORT:-8080}"
export CANARY_PORT="${CANARY_PORT:-8081}"

export WORKSPACE_BIN="${OHMYWORKPANEL_ROOT}/src-tauri/target/release/ohmyworkpanel-server"
export WORKSPACE_DIST="${OHMYWORKPANEL_ROOT}/dist"
