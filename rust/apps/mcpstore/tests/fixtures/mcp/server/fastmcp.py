"""仅供 Rust CLI 集成测试使用的最小 FastMCP 替身。"""

from __future__ import annotations

import inspect
import json
import sys
from dataclasses import dataclass
from typing import Any, Callable


_LATEST_PROTOCOL_VERSION = "2025-11-25"


@dataclass
class _ToolDef:
    name: str
    description: str
    input_schema: dict[str, Any]
    func: Callable[..., Any]


@dataclass
class _ResourceDef:
    uri: str
    name: str
    description: str
    func: Callable[..., Any]


@dataclass
class _PromptDef:
    name: str
    description: str
    arguments: list[dict[str, Any]]
    func: Callable[..., Any]


class FastMCP:
    def __init__(self, name: str):
        self._name = name
        self._tools: dict[str, _ToolDef] = {}
        self._resources: dict[str, _ResourceDef] = {}
        self._prompts: dict[str, _PromptDef] = {}

    def tool(self):
        def decorator(func: Callable[..., Any]) -> Callable[..., Any]:
            self._tools[func.__name__] = _ToolDef(
                name=func.__name__,
                description=inspect.getdoc(func) or "",
                input_schema=_build_input_schema(func),
                func=func,
            )
            return func

        return decorator

    def resource(self, uri: str):
        def decorator(func: Callable[..., Any]) -> Callable[..., Any]:
            self._resources[uri] = _ResourceDef(
                uri=uri,
                name=func.__name__,
                description=inspect.getdoc(func) or "",
                func=func,
            )
            return func

        return decorator

    def prompt(self):
        def decorator(func: Callable[..., Any]) -> Callable[..., Any]:
            self._prompts[func.__name__] = _PromptDef(
                name=func.__name__,
                description=inspect.getdoc(func) or "",
                arguments=_build_prompt_arguments(func),
                func=func,
            )
            return func

        return decorator

    def run(self, transport: str) -> None:
        if transport != "stdio":
            raise ValueError(f"仅支持 stdio，实际值: {transport}")
        _serve_stdio(self)

    def _handle_request(self, request: dict[str, Any]) -> dict[str, Any]:
        method = request.get("method")
        params = request.get("params") or {}

        if method == "initialize":
            protocol_version = params.get("protocolVersion") or _LATEST_PROTOCOL_VERSION
            return {
                "protocolVersion": protocol_version,
                "capabilities": {
                    "tools": {},
                    "resources": {},
                    "prompts": {},
                },
                "serverInfo": {
                    "name": self._name,
                    "version": "0.1.0",
                },
                "instructions": "fixture fastmcp replacement",
            }

        if method == "ping":
            return {}

        if method == "tools/list":
            return {
                "tools": [
                    {
                        "name": tool.name,
                        "description": tool.description,
                        "inputSchema": tool.input_schema,
                    }
                    for tool in self._tools.values()
                ]
            }

        if method == "tools/call":
            tool_name = params.get("name")
            tool = self._tools.get(tool_name)
            if tool is None:
                raise ValueError(f"未知工具: {tool_name}")
            arguments = params.get("arguments") or {}
            result = tool.func(**arguments)
            return {
                "content": [{"type": "text", "text": str(result)}],
                "isError": False,
            }

        if method == "resources/list":
            return {
                "resources": [
                    {
                        "uri": resource.uri,
                        "name": resource.name,
                        "description": resource.description,
                        "mimeType": "text/plain",
                    }
                    for resource in self._resources.values()
                ]
            }

        if method == "resources/templates/list":
            return {"resourceTemplates": []}

        if method == "resources/read":
            uri = params.get("uri")
            resource = self._resources.get(uri)
            if resource is None:
                raise ValueError(f"未知资源: {uri}")
            result = resource.func()
            return {
                "contents": [
                    {
                        "uri": resource.uri,
                        "mimeType": "text/plain",
                        "text": str(result),
                    }
                ]
            }

        if method == "prompts/list":
            return {
                "prompts": [
                    {
                        "name": prompt.name,
                        "description": prompt.description,
                        "arguments": prompt.arguments,
                    }
                    for prompt in self._prompts.values()
                ]
            }

        if method == "prompts/get":
            prompt_name = params.get("name")
            prompt = self._prompts.get(prompt_name)
            if prompt is None:
                raise ValueError(f"未知 prompt: {prompt_name}")
            arguments = params.get("arguments") or {}
            result = prompt.func(**arguments)
            return {
                "description": prompt.description,
                "messages": [
                    {
                        "role": "user",
                        "content": {
                            "type": "text",
                            "text": str(result),
                        },
                    }
                ],
            }

        raise ValueError(f"未支持的方法: {method}")


def _build_input_schema(func: Callable[..., Any]) -> dict[str, Any]:
    signature = inspect.signature(func)
    properties: dict[str, Any] = {}
    required: list[str] = []

    for name, parameter in signature.parameters.items():
        properties[name] = {"type": _json_schema_type(parameter.annotation)}
        if parameter.default is inspect.Signature.empty:
            required.append(name)

    schema: dict[str, Any] = {
        "type": "object",
        "properties": properties,
    }
    if required:
        schema["required"] = required
    return schema


def _build_prompt_arguments(func: Callable[..., Any]) -> list[dict[str, Any]]:
    signature = inspect.signature(func)
    arguments: list[dict[str, Any]] = []
    for name, parameter in signature.parameters.items():
        entry = {
            "name": name,
            "required": parameter.default is inspect.Signature.empty,
        }
        arguments.append(entry)
    return arguments


def _json_schema_type(annotation: Any) -> str:
    if annotation in (int,):
        return "integer"
    if annotation in (float,):
        return "number"
    if annotation in (bool,):
        return "boolean"
    if annotation in (dict,):
        return "object"
    if annotation in (list, tuple):
        return "array"
    return "string"


def _serve_stdio(app: FastMCP) -> None:
    while True:
        incoming = _read_message()
        if incoming is None:
            return
        request, framing = incoming

        if "id" not in request:
            continue

        request_id = request["id"]
        try:
            result = app._handle_request(request)
            _write_message(
                {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": result,
                },
                framing,
            )
        except Exception as error:  # noqa: BLE001 - 测试夹具直接返回协议错误
            _write_message(
                {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "error": {
                        "code": -32603,
                        "message": str(error),
                    },
                },
                framing,
            )


def _read_message() -> tuple[dict[str, Any], str] | None:
    headers: dict[str, str] = {}
    first_line = sys.stdin.buffer.readline()
    if not first_line:
        return None
    stripped = first_line.strip()
    if stripped.startswith(b"{"):
        return json.loads(stripped.decode("utf-8")), "json-line"

    text = first_line.decode("utf-8").strip()
    if ":" in text:
        key, value = text.split(":", 1)
        headers[key.strip().lower()] = value.strip()

    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        if line in (b"\r\n", b"\n"):
            break
        text = line.decode("utf-8").strip()
        if ":" not in text:
            continue
        key, value = text.split(":", 1)
        headers[key.strip().lower()] = value.strip()

    content_length = int(headers["content-length"])
    payload = sys.stdin.buffer.read(content_length)
    return json.loads(payload.decode("utf-8")), "content-length"


def _write_message(message: dict[str, Any], framing: str) -> None:
    payload = json.dumps(message, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    if framing == "json-line":
        sys.stdout.buffer.write(payload + b"\n")
        sys.stdout.buffer.flush()
        return

    sys.stdout.buffer.write(f"Content-Length: {len(payload)}\r\n\r\n".encode("ascii"))
    sys.stdout.buffer.write(payload)
    sys.stdout.buffer.flush()
