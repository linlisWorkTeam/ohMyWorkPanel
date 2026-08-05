#!/usr/bin/env bash
# Watchdog: if production slot is inactive, start it (and auth proxy).
# Safe to run frequently from systemd timer. Never promotes or overwrites artifacts.
set -euo pipefail

PROD_UNIT=linlis-work-panel.service
PROXY_UNIT=linlis-work-panel-proxy.service
LOG_TAG=linlis-prod-watchdog

log() { logger -t "$LOG_TAG" -- "$*"; echo "$*"; }

if ! systemctl is-active --quiet "$PROD_UNIT"; then
  log "ALERT: $PROD_UNIT inactive — starting"
  systemctl start "$PROD_UNIT" || log "ERROR: failed to start $PROD_UNIT"
  sleep 1
  if systemctl is-active --quiet "$PROD_UNIT"; then
    log "recovered: $PROD_UNIT is active"
  else
    log "ERROR: $PROD_UNIT still inactive after start"
    systemctl status "$PROD_UNIT" --no-pager -l | head -30 || true
    exit 1
  fi
fi

if systemctl list-unit-files "$PROXY_UNIT" >/dev/null 2>&1; then
  if ! systemctl is-active --quiet "$PROXY_UNIT"; then
    log "ALERT: $PROXY_UNIT inactive — starting"
    systemctl start "$PROXY_UNIT" || log "ERROR: failed to start $PROXY_UNIT"
  fi
fi

# Cheap HTTP probe (do not fail timer on transient 5xx during boot)
code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 3 http://127.0.0.1:8080/ || echo 000)
if [[ "$code" != "200" ]]; then
  log "WARN: prod HTTP probe got ${code} (unit may still be starting)"
fi
