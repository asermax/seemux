# Design: Pi.dev Hook Integration

**Feature Spec**: [../../feature-specs/hooks/pi-dev-integration.md](../../feature-specs/hooks/pi-dev-integration.md)
**Status**: Current

## Purpose

This document details the implementation design and engineering choices for the pi.dev agent lifecycle integration. It covers the in-process jiti extension architecture, the async connection caching, parallel tool stashing, and the cross-plugin events bridge.

## Architecture & Data Flow

```
[Custom User Extension]
         │ (Emits "seemux:event")
         ▼
     [pi.events] (Global Pi Event Bus)
         │
         ▼ (Listens to "seemux:event")
  [seemux-pi.ts] (In-Process TypeScript Extension)
         │ (JSON-RPC 2.0 NDJSON over Net.Socket)
         ▼
 [Seemux Daemon] (Unix Socket Server)
```

## Key Decisions

### Global Event Bus Inter-Plugin Communication

**Choice**: Expose a generic `"seemux:event"` channel on the standard `pi.events` bus rather than forcing extensions to import or depend on each other.
**Why**: 
- Establishes a highly decoupled, fail-safe communication interface.
- Downstream custom extensions (like permission managers or desktop-notifications) can interact with Seemux's visual state natively using simple fire-and-forget events, with no knowledge of Unix socket files or environment variables.
- Promotes modularity and prevents compile-time or load-order dependencies in jiti.

### Parallel Tool Parameter Stashing

**Choice**: Stash tool arguments in an internal `Map` keyed by `event.toolCallId` during `tool_execution_start` and retrieve them during `tool_execution_end`.
**Why**:
- Pi's native `tool_execution_end` payload does not contain the original tool invocation arguments (it only provides the final result).
- Stashing the arguments by `toolCallId` guarantees that Seemux can inspect the tool parameters (such as the Bash command executed) during `agent.tool.post_use` or `agent.tool.failed` to trigger git branch/PR redetection.
- Using `toolCallId` ensures perfect reliability and thread-safety under parallel tool executions.

### Compile-Time File Bundling (`include_str!`)

**Choice**: Bundle the TypeScript source code of `seemux-pi/index.ts` into the Seemux binary at compile-time using Rust's `include_str!` macro, and write it recursively to `~/.pi/agent/extensions/seemux-pi.ts` on startup.
**Why**:
- Keeps the Seemux installation completely self-contained and zero-config.
- Users do not need to clone the repository or manually configure Pi extensions.
- Content-aware deployment (by checking file existence and comparing content hash/equality) prevents redundant writes, making startup extremely fast and idempotent.

## System Behavior

### Hardened Session Routing

When forwarding events from the `"seemux:event"` channel, the extension explicitly overrides the `session_id` after the parameter spread:
```typescript
pi.events.on("seemux:event", (data: any) => {
  if (data && typeof data.method === "string") {
    sendToSeemux(data.method, {
      ...(data.params || {}),
      session_id: seemuxSessionId // Written after spread to prevent caller override
    });
  }
});
```
This guarantees that custom user extensions can never forge or accidentally override the destination tab session ID, keeping multi-tab event routing fully isolated.
