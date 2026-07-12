#!/usr/bin/env python3
"""Run the MCPStore MCP server from the Python package entry point.

This module intentionally exposes only the MCP server runner surface used by
`uvx mcpstore`. Full CLI commands are distributed by the npm and curl native
binary installers.
"""

from __future__ import annotations

import argparse
import sys

from mcpstore._rust import start_mcp_server


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="mcpstore",
        description="Run the MCPStore MCP server. Full CLI commands are available from npm/curl installs.",
    )
    parser.add_argument("--config-path", help="Path to an MCPStore config file")
    parser.add_argument(
        "--source",
        choices=["local", "db"],
        default="local",
        help="Store source used by the runner",
    )
    parser.add_argument(
        "--transport",
        choices=["stdio", "streamable-http", "http"],
        default="stdio",
        help="MCP transport for the runner",
    )
    parser.add_argument("--host", default="127.0.0.1", help="Host for streamable-http transport")
    parser.add_argument("--port", type=int, default=18300, help="Port for streamable-http transport")
    parser.add_argument("--path", default="/mcp", help="HTTP path for streamable-http transport")
    parser.add_argument("--scope", choices=["store", "agent"], default="store", help="Operation scope")
    parser.add_argument("--agent", help="Agent ID when --scope agent is used")
    parser.add_argument("--service", help="Optional service name to expose")
    parser.add_argument("--session-key", help="MCPStore business session key")
    parser.add_argument(
        "--backend",
        choices=["memory", "redis", "openkeyv_memory", "openkeyv_redis"],
        help="Cache backend",
    )
    parser.add_argument("--redis-url", help="Redis URL when using the redis backend")
    parser.add_argument("--namespace", help="Cache namespace")
    parser.add_argument("--expose-service-tools", action="store_true", help="Expose service management tools")
    parser.add_argument("--expose-cache-tools", action="store_true", help="Expose cache management tools")
    parser.add_argument("--expose-event-tools", action="store_true", help="Expose event observability tools")
    parser.add_argument(
        "--expose-session-state-tools",
        action="store_true",
        help="Expose session state management tools",
    )
    parser.add_argument(
        "--expose-tool-transform-tools",
        action="store_true",
        help="Expose tool transform management tools",
    )
    parser.add_argument("--expose-openapi-tools", action="store_true", help="Expose OpenAPI import tools")
    return parser


def main(argv: list[str] | None = None) -> None:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.scope == "agent" and not args.agent:
        parser.error("--agent is required when --scope agent is used")
    if args.scope == "store" and args.agent:
        parser.error("--agent can only be used with --scope agent")

    transport = "streamable-http" if args.transport == "http" else args.transport

    try:
        start_mcp_server(
            transport=transport,
            scope=args.scope,
            agent=args.agent,
            service=args.service,
            host=args.host,
            port=args.port,
            path=args.path,
            config_path=args.config_path,
            source=args.source,
            session_key=args.session_key,
            backend=args.backend,
            redis_url=args.redis_url,
            namespace=args.namespace,
            expose_session_state_tools=args.expose_session_state_tools,
            expose_tool_transform_tools=args.expose_tool_transform_tools,
            expose_openapi_tools=args.expose_openapi_tools,
            expose_service_tools=args.expose_service_tools,
            expose_cache_tools=args.expose_cache_tools,
            expose_event_tools=args.expose_event_tools,
        )
    except KeyboardInterrupt:
        raise SystemExit(130) from None
    except Exception as error:
        print(f"[error] MCPStore MCP runner failed to start: {error}", file=sys.stderr)
        raise SystemExit(1) from error


if __name__ == "__main__":
    main()
