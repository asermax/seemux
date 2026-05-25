import * as net from "node:net";

export default function (pi: any) {
  const socketPath = process.env.SEEMUX_SOCKET;
  const seemuxSessionId = process.env.SEEMUX_SESSION_ID;

  // Skip if not running inside Seemux
  if (!socketPath || !seemuxSessionId) return;

  const activeToolCalls = new Map<string, any>();

  function sendToSeemux(method: string, params: any) {
    const message = JSON.stringify({
      jsonrpc: "2.0",
      method,
      params
    }) + "\n";

    const client = net.connect(socketPath!, () => {
      client.write(message, () => {
        client.end();
      });
    });

    client.on("error", () => {
      // Ignore errors silently (e.g. if seemux is closed or socket is unavailable)
    });
  }

  // --- Core Lifecycle Hooks ---

  pi.on("session_start", async (_event: any, ctx: any) => {
    const agent_session_id = typeof ctx?.sessionManager?.getSessionId === "function"
      ? ctx.sessionManager.getSessionId()
      : (ctx?.sessionId || "");

    sendToSeemux("agent.session.started", {
      session_id: seemuxSessionId,
      pid: process.pid,
      agent_session_id,
      provider: "pi",
      binary: "pi"
    });
  });

  pi.on("agent_start", async (_event: any, _ctx: any) => {
    sendToSeemux("agent.prompt.submitted", {
      session_id: seemuxSessionId
    });
  });

  pi.on("tool_execution_start", async (event: any, _ctx: any) => {
    if (event.toolCallId) {
      activeToolCalls.set(event.toolCallId, event.args || {});
    }

    sendToSeemux("agent.tool.pre_use", {
      session_id: seemuxSessionId,
      tool_name: event.toolName,
      tool_input: event.args || {}
    });
  });

  pi.on("tool_execution_end", async (event: any, _ctx: any) => {
    const args = event.toolCallId ? activeToolCalls.get(event.toolCallId) : undefined;
    if (event.toolCallId) {
      activeToolCalls.delete(event.toolCallId);
    }

    const method = event.isError ? "agent.tool.failed" : "agent.tool.post_use";
    sendToSeemux(method, {
      session_id: seemuxSessionId,
      tool_name: event.toolName,
      tool_input: args || {}
    });
  });

  pi.on("agent_end", async (event: any, _ctx: any) => {
    const lastAssistantMsg = [...(event.messages || [])]
      .reverse()
      .find(m => m.role === "assistant");

    let last_message = "Task completed";
    let isError = false;

    if (lastAssistantMsg) {
      if (lastAssistantMsg.stopReason === "error") {
        isError = true;
      }
      if (lastAssistantMsg.content) {
        const textBlock = lastAssistantMsg.content.find((c: any) => c.type === "text" && typeof c.text === "string");
        if (textBlock) {
          last_message = textBlock.text;
        }
      }
    }

    const method = isError ? "agent.response.failed" : "agent.response.completed";
    sendToSeemux(method, {
      session_id: seemuxSessionId,
      last_message
    });
  });

  pi.on("session_shutdown", async (_event: any, _ctx: any) => {
    activeToolCalls.clear();
    sendToSeemux("agent.session.ended", {
      session_id: seemuxSessionId
    });
  });

  // --- Extensibility & Custom Callbacks ---

  if (pi.events && typeof pi.events.on === "function") {
    // Register a generic, Decoupled Event Channel
    // Other extensions can simply do: pi.events.emit("seemux:event", { method: "agent.attention.requested", params: { message: "..." } })
    pi.events.on("seemux:event", (data: any) => {
      if (data && typeof data.method === "string") {
        sendToSeemux(data.method, {
          ...(data.params || {}),
          session_id: seemuxSessionId
        });
      }
    });
  }
}
