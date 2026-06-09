#!/usr/bin/env python3
"""
MCPStore CLI thin wrapper.

Python 入口不再承载真实 CLI 逻辑，只负责定位并执行 Rust CLI。
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

from mcpstore._rust_cli import resolve_rust_cli_binary, resolve_runtime_cwd


def _repo_root() -> Path:
    return Path(resolve_runtime_cwd())


def _resolve_rust_cli_binary() -> str:
    return resolve_rust_cli_binary()


def _build_rust_cli_command(argv: list[str]) -> list[str]:
    return [_resolve_rust_cli_binary(), *argv]


def run_rust_cli(argv: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(
        _build_rust_cli_command(argv),
        check=False,
        cwd=str(_repo_root()),
    )


def main() -> None:
    try:
        completed = run_rust_cli(sys.argv[1:])
    except KeyboardInterrupt:
        raise SystemExit(130) from None
    except Exception as error:
        print(f"[错误] Rust CLI 启动失败: {error}", file=sys.stderr)
        raise SystemExit(1) from error

    raise SystemExit(completed.returncode)


if __name__ == "__main__":
    main()
