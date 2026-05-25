#!/usr/bin/env bash
# seemux hook script — called by Claude Code hooks via the seemux-hooks plugin
# Reads JSON from stdin and sends it to seemux via Unix socket

# Skip if not running inside a seemux terminal
[[ -z "$SEEMUX_SOCKET" ]] && exit 0

PAYLOAD=$(cat)
if [[ -z "$PAYLOAD" ]]; then
    PAYLOAD="{}"
fi

case "$1" in
    "session-start")
        METHOD="agent.session.started"
        ;;
    "prompt-submit")
        METHOD="agent.prompt.submitted"
        ;;
    "pre-tool-use")
        METHOD="agent.tool.pre_use"
        ;;
    "post-tool-use")
        METHOD="agent.tool.post_use"
        ;;
    "post-tool-use-failure")
        METHOD="agent.tool.failed"
        ;;
    "notification")
        METHOD="agent.attention.requested"
        ;;
    "stop")
        METHOD="agent.response.completed"
        ;;
    "stop-failure")
        METHOD="agent.response.failed"
        ;;
    "session-end")
        METHOD="agent.session.ended"
        ;;
    *)
        METHOD="$1"
        ;;
esac

if [[ "$METHOD" == "agent.session.started" ]]; then
    JSON_MSG=$(echo "$PAYLOAD" | jq -c --arg sid "$SEEMUX_SESSION_ID" '
        {
            jsonrpc: "2.0",
            method: "agent.session.started",
            params: {
                session_id: $sid,
                pid: .pid,
                agent_session_id: .session_id,
                provider: "claude",
                binary: "claude"
            }
        }
    ')
else
    JSON_MSG=$(echo "$PAYLOAD" | jq -c --arg sid "$SEEMUX_SESSION_ID" --arg method "$METHOD" '
        {
            jsonrpc: "2.0",
            method: $method,
            params: (if type == "object" then . else {} end + {session_id: $sid})
        }
    ')
fi

printf '%s\n' "$JSON_MSG" | socat - UNIX-CONNECT:"$SEEMUX_SOCKET" 2>/dev/null || true
