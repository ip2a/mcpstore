"""Path helpers for the Python compatibility configuration surface."""

from __future__ import annotations

from pathlib import Path


def get_user_data_dir() -> Path:
    return Path.home() / ".mcpstore"


def get_user_default_mcp_path() -> Path:
    return get_user_data_dir() / "mcp.json"


def get_user_config_path() -> Path:
    return get_user_data_dir() / "config.toml"


def ensure_config_directory() -> Path:
    config_dir = get_user_data_dir()
    config_dir.mkdir(parents=True, exist_ok=True)
    return config_dir


def create_default_config_if_not_exists() -> bool:
    config_dir = ensure_config_directory()
    config_path = config_dir / "config.toml"
    if config_path.exists():
        return True
    config_path.write_text(
        "# MCPStore Python compatibility config\n"
        "# Runtime setup is owned by the Rust core via MCPStore.setup_store(...).\n",
        encoding="utf-8",
    )
    return True


def initialize_config_system() -> bool:
    return create_default_config_if_not_exists()
