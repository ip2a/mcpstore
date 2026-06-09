"""Rust-first MCP compatibility module.

`src/mcpstore/mcp/` 历史 Python MCP framework 已移除。
这里只保留最小兼容面，避免 Python 端继续承载真实 MCP 实现。
"""

from __future__ import annotations

import sys
from typing import Optional

from mcpstore.cli import main as cli_main
from mcpstore.core.store import MCPStore


_REMOVED_EXPORTS = {
    "Client": "Python MCP client 已移除；请改用 Rust `mcpstore` 二进制或 Rust API。",
    "Context": "Python MCP context 已移除；请改用 Rust `MCPStore` 上下文能力。",
    "MCPKit": "Python MCP server framework 已移除；请改用 Rust 端实现。",
    "Settings": "历史 Python MCP settings 已移除；不再支持该入口。",
    "settings": "历史 Python MCP settings 已移除；不再支持该入口。",
}


def run_server(
    *,
    config_path: Optional[str] = None,
    scope: str = "store",
    agent: Optional[str] = None,
) -> None:
    """启动 Rust MCP server。"""

    argv = ["mcp-server"]
    if config_path:
        argv.extend(["--config-path", config_path])
    if scope != "store":
        argv.extend(["--scope", scope])
    if agent:
        argv.extend(["--agent", agent])

    completed = cli_main.run_rust_cli(argv)
    if completed.returncode != 0:
        raise RuntimeError(f"Rust MCP server 启动失败，退出码: {completed.returncode}")


def main(argv: Optional[list[str]] = None) -> None:
    completed = cli_main.run_rust_cli(["mcp-server", *(argv or sys.argv[1:])])
    raise SystemExit(completed.returncode)


def __getattr__(name: str):
    if name == "MCPStore":
        return MCPStore

    if name in _REMOVED_EXPORTS:
        raise ImportError(_REMOVED_EXPORTS[name])

    raise AttributeError(f"module '{__name__}' has no attribute '{name}'")


__all__ = ["MCPStore", "run_server", "main"]


if __name__ == "__main__":
    main()
