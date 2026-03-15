#!/usr/bin/env bash
# seemux hook script — called by Claude Code hooks via the seemux-hooks plugin
# Reads JSON from stdin and sends it to seemux via Unix socket

# Skip if not running inside a seemux terminal
[[ -z "$SEEMUX_SOCKET" ]] && exit 0

PAYLOAD=$(cat)
printf '{"event":"%s","session_id":"%s","payload":%s}\n' "$1" "$SEEMUX_SESSION_ID" "$PAYLOAD" \
    | socat - UNIX-CONNECT:"$SEEMUX_SOCKET" 2>/dev/null || true
