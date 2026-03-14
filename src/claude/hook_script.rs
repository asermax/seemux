use std::path::PathBuf;

pub fn hook_script(socket_path: &PathBuf) -> String {
    format!(
        r#"#!/usr/bin/env bash
# seemux hook script — called by Claude Code hooks
# Reads JSON from stdin and sends to seemux via Unix socket

PAYLOAD=$(cat)
printf '{{"event":"%s","session_id":"%s","payload":%s}}\n' "$1" "$SEEMUX_SESSION_ID" "$PAYLOAD" \
    | socat - UNIX-CONNECT:"{socket_path}" 2>/dev/null || true
"#,
        socket_path = socket_path.display()
    )
}

pub fn claude_wrapper(_bin_dir: &PathBuf, hook_script_path: &PathBuf) -> String {
    format!(
        r##"#!/usr/bin/env bash
# seemux claude wrapper — intercepts claude invocations to inject hooks

# Find real claude binary, skipping our wrapper directory
find_real_claude() {{
    local self_dir
    self_dir="$(cd "$(dirname "$0")" && pwd)"
    local IFS=:
    for d in $PATH; do
        [[ "$d" == "$self_dir" ]] && continue
        [[ -x "$d/claude" ]] && printf '%s' "$d/claude" && return 0
    done
    return 1
}}

# Pass through if not in a seemux terminal
if [[ -z "$SEEMUX_SOCKET" ]]; then
    REAL_CLAUDE="$(find_real_claude)" || {{ echo "Error: claude not found in PATH" >&2; exit 127; }}
    exec "$REAL_CLAUDE" "$@"
fi

REAL_CLAUDE="$(find_real_claude)" || {{ echo "Error: claude not found in PATH" >&2; exit 127; }}

# Pass through subcommands that don't support session/hook flags
case "${{1:-}}" in
    mcp|config|api-key) exec "$REAL_CLAUDE" "$@" ;;
esac

# Unset CLAUDECODE to avoid nested session detection
unset CLAUDECODE

# Check if user already specified session/resume flags
SKIP_SESSION_ID=false
for arg in "$@"; do
    case "$arg" in
        --resume|--resume=*|--session-id|--session-id=*|--continue|-c)
            SKIP_SESSION_ID=true
            break
            ;;
    esac
done

HOOK_SCRIPT="{hook_script}"

HOOKS_JSON='{{"hooks":{{"SessionStart":[{{"matcher":"","hooks":[{{"type":"command","command":"'"$HOOK_SCRIPT"' session-start","timeout":10}}]}}],"Stop":[{{"matcher":"","hooks":[{{"type":"command","command":"'"$HOOK_SCRIPT"' stop","timeout":10}}]}}],"Notification":[{{"matcher":"","hooks":[{{"type":"command","command":"'"$HOOK_SCRIPT"' notification","timeout":10}}]}}],"UserPromptSubmit":[{{"matcher":"","hooks":[{{"type":"command","command":"'"$HOOK_SCRIPT"' prompt-submit","timeout":10}}]}}],"PreToolUse":[{{"matcher":"","hooks":[{{"type":"command","command":"'"$HOOK_SCRIPT"' pre-tool-use","timeout":5,"async":true}}]}}],"SessionEnd":[{{"matcher":"","hooks":[{{"type":"command","command":"'"$HOOK_SCRIPT"' session-end","timeout":1}}]}}]}}}}'

if [[ "$SKIP_SESSION_ID" == true ]]; then
    exec "$REAL_CLAUDE" --settings "$HOOKS_JSON" "$@"
else
    SESSION_ID="$(uuidgen | tr '[:upper:]' '[:lower:]')"
    exec "$REAL_CLAUDE" --session-id "$SESSION_ID" --settings "$HOOKS_JSON" "$@"
fi
"##,
        hook_script = hook_script_path.display(),
    )
}
