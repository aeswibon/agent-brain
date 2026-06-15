#!/usr/bin/env python3
"""Cursor hook: require agent-brain route_task before other tools each user turn."""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

STATE_PATH = (
    Path(os.environ.get("AGENT_BRAIN_HOME", Path.home() / ".agent_brain"))
    / "hooks"
    / "route_state.json"
)

ROUTE_TOOL_NAMES = {"route_task", "MCP:route_task", "mcp:route_task"}


def disabled() -> bool:
    v = os.environ.get("AGENT_BRAIN_ROUTE_HOOKS", "1").strip().lower()
    return v in {"0", "false", "no", "off"}


def load_state() -> dict:
    if not STATE_PATH.exists():
        return {}
    try:
        return json.loads(STATE_PATH.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return {}


def save_state(state: dict) -> None:
    STATE_PATH.parent.mkdir(parents=True, exist_ok=True)
    STATE_PATH.write_text(json.dumps(state), encoding="utf-8")


def is_agent_brain_command(event: dict) -> bool:
    cmd = str(event.get("command") or "")
    server = str(event.get("server") or "")
    return "agent-brain" in cmd or server == "agent-brain"


def is_route_task(event: dict) -> bool:
    tool = str(event.get("tool_name") or "")
    if tool in ROUTE_TOOL_NAMES or tool.endswith(":route_task"):
        return True
    if tool == "route_task" and is_agent_brain_command(event):
        return True
    # preToolUse MCP tools may only expose MCP:route_task without server field
    return tool in ROUTE_TOOL_NAMES


def deny_payload() -> dict:
    return {
        "permission": "deny",
        "agent_message": (
            "You must call agent-brain MCP tool route_task first with the user's "
            "message (and cwd/open_files) before any other tool. Then use the "
            "returned skills, rules, and memory."
        ),
        "user_message": "agent-brain hook: call route_task before other tools.",
    }


def handle_before_submit_prompt(_event: dict) -> dict:
    save_state({"needs_route": True})
    return {"continue": True}


def handle_after_mcp_execution(event: dict) -> dict:
    if is_route_task(event) and (is_agent_brain_command(event) or event.get("tool_name") in ROUTE_TOOL_NAMES):
        state = load_state()
        state["needs_route"] = False
        if event.get("generation_id"):
            state["generation_id"] = event["generation_id"]
        save_state(state)
    return {}


def handle_pre_tool_use(event: dict) -> dict:
    if is_route_task(event):
        return {"permission": "allow"}
    state = load_state()
    if state.get("needs_route"):
        return deny_payload()
    return {"permission": "allow"}


def handle_before_mcp_execution(event: dict) -> dict:
    if is_route_task(event) and is_agent_brain_command(event):
        return {"permission": "allow"}
    if is_route_task(event):
        # route_task from agent-brain even if command field is missing
        return {"permission": "allow"}
    state = load_state()
    if state.get("needs_route"):
        return deny_payload()
    return {"permission": "allow"}


def main() -> int:
    if disabled():
        event_name = ""
        try:
            event = json.load(sys.stdin)
            event_name = event.get("hook_event_name", "")
        except json.JSONDecodeError:
            event = {}
        if event_name == "beforeSubmitPrompt":
            print(json.dumps({"continue": True}))
        elif event_name in {"preToolUse", "beforeMCPExecution", "beforeShellExecution"}:
            print(json.dumps({"permission": "allow"}))
        else:
            print("{}")
        return 0

    try:
        event = json.load(sys.stdin)
    except json.JSONDecodeError:
        print(json.dumps({"permission": "allow"}))
        return 0

    name = event.get("hook_event_name", "")

    if name == "beforeSubmitPrompt":
        out = handle_before_submit_prompt(event)
    elif name == "afterMCPExecution":
        out = handle_after_mcp_execution(event)
    elif name == "preToolUse":
        out = handle_pre_tool_use(event)
    elif name == "beforeMCPExecution":
        out = handle_before_mcp_execution(event)
    else:
        out = {}

    print(json.dumps(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
