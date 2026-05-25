# Pi.dev Hook Integration Spec

## Overview

The pi.dev hook integration provides full status, notification, and restart-resume parity for pi.dev sessions running in Seemux tabs. It achieves this by deploying a native TypeScript extension (`plugins/seemux-pi/index.ts`) that runs inside Pi's jiti runtime, intercepts Pi's lifecycle events, and translates them into the canonical JSON-RPC 2.0 lifecycle contract over Seemux's Unix socket.

The integration is designed to be fully decoupled and extensible. Rather than hardcoding custom attention or notification logic into the core extension, it exposes a global decoupled event channel (`"seemux:event"`) over Pi's event bus. Other custom Pi extensions (like permission or notification managers) can emit messages directly to this channel to trigger any Seemux visual state mutation dynamically.

## Requirements

| ID | Requirement |
|----|-------------|
| R1 | Automatically deploy the bundled Pi extension (`plugins/seemux-pi/index.ts`) to `~/.pi/agent/extensions/seemux-pi.ts` at Seemux startup |
| R2 | Perform content-aware deployment: only overwrite the local file if it is missing or differs from the bundled content, avoiding redundant writes |
| R3 | Map native Pi lifecycle events (`session_start`, `agent_start`, `tool_execution_start`, `tool_execution_end`, `agent_end`, `session_shutdown`) to standard JSON-RPC 2.0 envelopes and canonical methods |
| R4 | Correctly track active tool execution arguments by `toolCallId` to enrich `agent.tool.post_use` and `agent.tool.failed` payloads with `tool_name` and `tool_input` (enabling precise asynchronous git branch redetection) |
| R5 | Conditionally map failed agent turns (`stopReason === "error"`) to `agent.response.failed` and successful runs to `agent.response.completed` in the `agent_end` event |
| R6 | Expose the global decoupled event channel `"seemux:event"` on the `pi.events` bus, forwarding any custom method and parameters safely to the Seemux Unix socket |
| R7 | Hard-code `session_id: seemuxSessionId` on `"seemux:event"` forwards after parameter spreads to prevent external extensions from overriding routing |
| R8 | Fail open silently when running outside Seemux or when socket writes encounter errors, ensuring the extension never blocks Pi |

## Mappings

The Pi extension maps native events to the canonical socket contract as follows:

| Pi Native Event | Canonical JSON-RPC Method | Extracted Parameters |
|-----------------|---------------------------|----------------------|
| `session_start` | `agent.session.started` | `pid`, `agent_session_id`, `provider: "pi"`, `binary: "pi"` |
| `agent_start` | `agent.prompt.submitted` | (empty params) |
| `tool_execution_start` | `agent.tool.pre_use` | `tool_name`, `tool_input: event.args` |
| `tool_execution_end` | `agent.tool.post_use` or `agent.tool.failed` | `tool_name`, `tool_input: (stashed args)` |
| `agent_end` | `agent.response.completed` or `agent.response.failed` | `last_message` (from assistant's text content block) |
| `session_shutdown` | `agent.session.ended` | (empty params) |

## Extensibility API

Other custom extensions running in the same Pi process can use the decoupled channel to broadcast arbitrary Seemux notifications or status updates.

### Triggering Custom Attention Badge
```typescript
pi.events.emit("seemux:event", {
  method: "agent.attention.requested",
  params: {
    event_name: "custom_alert",
    message: "Custom notification message"
  }
});
```

### Triggering Status Resets
```typescript
pi.events.emit("seemux:event", {
  method: "agent.prompt.submitted"
});
```
