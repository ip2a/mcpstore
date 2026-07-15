import json
import os
import sys
import threading
import time

write_lock = threading.Lock()
cancelled = threading.Event()
marker_path = os.environ.get("MCP_EXECUTION_MARKER")


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


def handle(message):
    method = message.get("method")
    request_id = message.get("id")
    params = message.get("params") or {}

    if method == "initialize":
        respond(
            request_id,
            {
                "protocolVersion": params.get("protocolVersion"),
                "capabilities": {"tools": {"listChanged": False}},
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
