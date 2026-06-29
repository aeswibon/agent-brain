/**
 * OpenCode plugin: require agent-brain route_task before other agent-brain MCP tools.
 * Loads from ~/.config/opencode/plugin/ or .opencode/plugin/
 */
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

function routeGateScript(): string | null {
  const candidates = [
    join(homedir(), ".config", "opencode", "hooks", "agent-brain", "route_gate.py"),
    join(process.cwd(), ".opencode", "hooks", "agent-brain", "route_gate.py"),
  ];
  for (const path of candidates) {
    if (existsSync(path)) return path;
  }
  return null;
}

function isRouteTask(tool: string): boolean {
  const t = tool.toLowerCase();
  return (
    t === "route_task" ||
    t.endsWith("_route_task") ||
    t.endsWith(":route_task") ||
    (t.includes("route_task") &&
      (t.includes("agent-brain") || t.includes("agent_brain")))
  );
}

function isAgentBrainTool(tool: string): boolean {
  const t = tool.toLowerCase();
  if (isRouteTask(tool)) return true;
  return (
    t.startsWith("agent-brain_") ||
    t.startsWith("agent_brain_") ||
    t.startsWith("mcp_agent-brain_") ||
    t.startsWith("mcp_agent_brain_") ||
    t.startsWith("mcp__agent-brain__") ||
    t.startsWith("mcp__agent_brain__") ||
    (t.includes("agent-brain") && t.includes("mcp")) ||
    (t.includes("agent_brain") && t.includes("mcp"))
  );
}

function gateMissingPayload(tool: string): Record<string, unknown> {
  return {
    permission: "deny",
    agent_message:
      "agent-brain route gate script missing. Run: agent-brain install --opencode --global",
  };
}

function gateErrorPayload(message: string): Record<string, unknown> {
  return {
    permission: "deny",
    agent_message: `agent-brain route gate error: ${message}`,
  };
}

function runGate(event: Record<string, unknown>): Record<string, unknown> {
  const tool = String(event.tool_name ?? "");
  const brainTool = isAgentBrainTool(tool);
  const script = routeGateScript();

  if (!script) {
    if (brainTool && !isRouteTask(tool)) {
      return gateMissingPayload(tool);
    }
    return { permission: "allow" };
  }

  const res = spawnSync("python3", [script], {
    input: JSON.stringify({
      server: "agent-brain",
      ...event,
    }),
    encoding: "utf-8",
    timeout: 25_000,
  });

  if (res.error) {
    if (brainTool && !isRouteTask(tool)) {
      return gateErrorPayload(res.error.message ?? "python3 failed");
    }
    return { permission: "allow" };
  }

  const out = (res.stdout || "").trim();
  if (!out) return {};
  try {
    return JSON.parse(out) as Record<string, unknown>;
  } catch {
    if (brainTool && !isRouteTask(tool)) {
      return gateErrorPayload("invalid JSON from route_gate.py");
    }
    return {};
  }
}

function gateDenied(out: Record<string, unknown>): boolean {
  if (out.permission === "deny") return true;
  if (out.decision === "deny" || out.decision === "block") return true;
  const hs = out.hookSpecificOutput as Record<string, unknown> | undefined;
  return hs?.permissionDecision === "deny";
}

function denyMessage(out: Record<string, unknown>): string {
  return (
    (out.agent_message as string) ||
    (out.reason as string) ||
    (out.systemMessage as string) ||
    "Call agent-brain route_task first (user_message, cwd, open_files)."
  );
}

export const AgentBrainRouteGate = async () => {
  return {
    "tool.execute.before": async (
      input: { tool: string; sessionID?: string; callID?: string },
      output: { args: Record<string, unknown> },
    ) => {
      void output;
      const out = runGate({
        hook_event_name: "PreToolUse",
        tool_name: input.tool,
        tool_input: output.args,
        session_id: input.sessionID,
        call_id: input.callID,
      });
      if (gateDenied(out)) {
        throw new Error(denyMessage(out));
      }
    },
    "tool.execute.after": async (
      input: { tool: string; sessionID?: string; callID?: string },
      output: { args?: Record<string, unknown>; error?: unknown },
    ) => {
      runGate({
        hook_event_name: "PostToolUse",
        tool_name: input.tool,
        tool_input: output.args ?? {},
        server: "agent-brain",
        success: output.error == null,
        session_id: input.sessionID,
        call_id: input.callID,
      });
    },
    "chat.message": async (
      input: { role?: string },
      _output: unknown,
    ) => {
      if (input.role && input.role !== "user") return;
      runGate({ hook_event_name: "UserPromptSubmit" });
    },
  };
};

export default AgentBrainRouteGate;
