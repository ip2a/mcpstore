import json
import os
import sys
import threading
import time

write_lock = threading.Lock()
cancelled = threading.Event()
marker_path = os.environ.get("MCP_EXECUTION_MARKER")
elicitation_lock = threading.Lock()
pending_elicitations = {}
next_elicitation_id = 1


def mark(message):
    if not marker_path:
        return
    with open(marker_path, "a", encoding="utf-8") as marker:
        marker.write(message + "\n")
        marker.flush()


def send(message):
    encoded = json.dumps(message, separators=(",", ":"))
    with write_lock:
        sys.stdout.write(encoded + "\n")
        sys.stdout.flush()


def respond(request_id, result):
    send({"jsonrpc": "2.0", "id": request_id, "result": result})


def progress_token(params):
    metadata = params.get("_meta") or {}
    return metadata.get("progressToken", metadata.get("progress_token"))


def run_tool(request_id, params):
    name = params.get("name")
    token = progress_token(params)
    mark("call_started:" + str(name))
    if name == "progress":
        for step in (1, 2):
            time.sleep(0.05)
            send(
                {
                    "jsonrpc": "2.0",
                    "method": "notifications/progress",
                    "params": {
                        "progressToken": token,
                        "progress": step,
                        "total": 2,
                        "message": "fixture-progress",
                    },
                }
            )
        respond(
            request_id,
            {
                "content": [{"type": "text", "text": "fixture-complete"}],
                "isError": False,
            },
        )
        return

    if name == "tool_error":
        respond(
            request_id,
            {
                "content": [{"type": "text", "text": "fixture-tool-error"}],
                "isError": True,
            },
        )
        return

    if name in {
        "elicit_form",
        "elicit_url",
        "elicit_url_unsafe_scheme",
        "elicit_url_credentials",
        "elicit_task_form",
    }:
        result = request_elicitation(name)
        if result is None:
            cancelled.wait(5)
            return
        action = result.get("action")
        if action == "decline":
            respond(
                request_id,
                {
                    "content": [{"type": "text", "text": "elicitation-declined"}],
                    "isError": False,
                },
            )
            return
        if action != "accept":
            cancelled.wait(5)
            return
        if name == "elicit_task_form":
            respond(
                request_id,
                {
                    "task": {
                        "taskId": "fixture-elicitation-task",
                        "status": "working",
                        "createdAt": "2026-01-01T00:00:00Z",
                        "lastUpdatedAt": "2026-01-01T00:00:00Z",
                        "ttl": None,
                    }
                },
            )
        else:
            respond(
                request_id,
                {
                    "content": [{"type": "text", "text": "elicitation-accepted"}],
                    "isError": False,
                },
            )
        return

    if name == "keep_progress":
        step = 0
        while not cancelled.wait(0.1):
            step += 1
            send(
                {
                    "jsonrpc": "2.0",
                    "method": "notifications/progress",
                    "params": {
                        "progressToken": token,
                        "progress": step,
                        "message": "fixture-still-working",
                    },
                }
            )
        return

    while not cancelled.wait(0.05):
        pass


def request_elicitation(name):
    global next_elicitation_id
    with elicitation_lock:
        request_id = "fixture-elicit-" + str(next_elicitation_id)
        next_elicitation_id += 1
        completed = threading.Event()
        pending_elicitations[request_id] = {"event": completed, "result": None}

    if name in {"elicit_form", "elicit_task_form"}:
        params = {
            "mode": "form",
            "message": "Provide the requested account data",
            "requestedSchema": {
                "type": "object",
                "properties": {
                    "secret": {"type": "string", "minLength": 1},
                    "count": {"type": "integer", "minimum": 1},
                },
                "required": ["secret", "count"],
            },
        }
    else:
        url = "https://example.com/continue"
        if name == "elicit_url_unsafe_scheme":
            url = "file:///tmp/secret"
        elif name == "elicit_url_credentials":
            url = "https://user:secret@example.com/continue"
        params = {
            "mode": "url",
            "message": "Continue externally",
            "url": url,
            "elicitationId": "fixture-handoff-1",
        }

    send({"jsonrpc": "2.0", "id": request_id, "method": "elicitation/create", "params": params})
    completed.wait(10)
    with elicitation_lock:
        entry = pending_elicitations.pop(request_id, None)
    result = entry.get("result") if entry else None
    if result is not None:
        mark("elicitation_response:" + name + ":" + str(result.get("action", "")))
    return result


def handle(message):
    method = message.get("method")
    request_id = message.get("id")
    params = message.get("params") or {}

    if method is None and request_id is not None:
        with elicitation_lock:
            entry = pending_elicitations.get(str(request_id))
            if entry is not None:
                entry["result"] = message.get("result") or {}
                entry["event"].set()
                return

    if method == "initialize":
        respond(
            request_id,
            {
                "protocolVersion": params.get("protocolVersion"),
                "capabilities": {
                    "tools": {"listChanged": False},
                    "tasks": {
                        "list": {},
                        "cancel": {},
                        "requests": {"tools": {"call": {}}},
                    },
                },
                "serverInfo": {"name": "execution-cli-fixture", "version": "1.0.0"},
            },
        )
    elif method == "ping":
        respond(request_id, {})
    elif method == "tools/list":
        respond(
            request_id,
            {
                "tools": [
                    {
                        "name": "progress",
                        "description": "Emit progress and complete",
                        "inputSchema": {"type": "object"},
                    },
                    {
                        "name": "hang",
                        "description": "Wait until cancelled",
                        "inputSchema": {"type": "object"},
                    },
                    {
                        "name": "tool_error",
                        "description": "Return an MCP tool error result",
                        "inputSchema": {"type": "object"},
                    },
                    {
                        "name": "keep_progress",
                        "description": "Emit progress until cancelled",
                        "inputSchema": {"type": "object"},
                    },
                    {
                        "name": "elicit_form",
                        "description": "Request form elicitation",
                        "inputSchema": {"type": "object"},
                    },
                    {
                        "name": "elicit_url",
                        "description": "Request URL elicitation",
                        "inputSchema": {"type": "object"},
                    },
                    {
                        "name": "elicit_url_unsafe_scheme",
                        "description": "Request an unsafe URL elicitation",
                        "inputSchema": {"type": "object"},
                    },
                    {
                        "name": "elicit_url_credentials",
                        "description": "Request a credential-bearing URL elicitation",
                        "inputSchema": {"type": "object"},
                    },
                    {
                        "name": "elicit_task_form",
                        "description": "Request form elicitation in a task call",
                        "inputSchema": {"type": "object"},
                    },
                ]
            },
        )
    elif method == "tools/call":
        threading.Thread(target=run_tool, args=(request_id, params), daemon=True).start()
    elif method == "notifications/cancelled":
        reason = params.get("reason")
        mark("cancelled:" + (reason or ""))
        cancelled.set()


for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    handle(json.loads(line))
