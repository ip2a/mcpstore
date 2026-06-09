"""Rust CLI resolution helpers for the Python console entry point."""

from __future__ import annotations

import os
from pathlib import Path


def _binary_name() -> str:
    return "mcpstore.exe" if os.name == "nt" else "mcpstore"


def _package_root() -> Path:
    return Path(__file__).resolve().parent


def _is_repo_root(path: Path) -> bool:
    if not (path / "rust" / "Cargo.toml").exists():
        return False
    return (path / "python" / "pyproject.toml").exists() or (path / "pyproject.toml").exists()


def _find_repo_root() -> Path | None:
    for parent in _package_root().parents:
        if _is_repo_root(parent):
            return parent
    return None


def resolve_runtime_cwd() -> str:
    repo_root = _find_repo_root()
    if repo_root is not None:
        return str(repo_root)
    return str(Path.cwd())


def resolve_rust_cli_binary() -> str:
    env_path = os.getenv("MCPSTORE_RUST_BIN")
    candidates: list[Path] = []
    if env_path:
        candidates.append(Path(env_path))

    candidates.append(_package_root() / "bin" / _binary_name())

    repo_root = _find_repo_root()
    if repo_root is not None:
        candidates.extend(
            [
                repo_root / "target" / "debug" / _binary_name(),
                repo_root / "target" / "release" / _binary_name(),
                repo_root / "rust" / "target" / "debug" / _binary_name(),
                repo_root / "rust" / "target" / "release" / _binary_name(),
            ]
        )

    checked = []
    for candidate in candidates:
        checked.append(str(candidate))
        if not candidate.exists():
            continue
        if not os.access(candidate, os.X_OK):
            raise RuntimeError(f"Rust CLI 不可执行: {candidate}")
        return str(candidate)

    raise RuntimeError(
        "未找到 Rust CLI 二进制。请先执行 `cargo build -p mcpstore_cli`、"
        "使用随 wheel 分发的内置二进制，或设置 MCPSTORE_RUST_BIN 指向可执行的 mcpstore。"
        f" 已检查路径: {checked}"
    )
