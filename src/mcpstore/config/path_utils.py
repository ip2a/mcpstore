"""
Path utilities for MCPStore configuration
"""

from pathlib import Path


def get_user_default_mcp_path() -> Path:
    """
    Get the default user-level mcp.json path
    
    Returns:
        Path: User-level default path (~/.mcpstore/mcp.json)
    """
    return Path.home() / ".mcpstore" / "mcp.json"


def get_user_data_dir() -> Path:
    """
    Get the user-level data directory

    Returns:
        Path: User-level data directory (~/.mcpstore)
    """
    return Path.home() / ".mcpstore"


def get_user_config_path() -> Path:
    """
    Get the user-level config.toml path

    Returns:
        Path: User-level config path (~/.mcpstore/config.toml)
    """
    return Path.home() / ".mcpstore" / "config.toml"
