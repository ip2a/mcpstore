"""Skills providers for exposing agent skills as MCP resources.

This module provides a two-layer architecture for skill discovery:

- **SkillProvider**: Handles a single skill folder, exposing its files as resources.
- **SkillsDirectoryProvider**: Scans a directory, creates a SkillProvider per folder.
- **Vendor providers**: Platform-specific providers for Claude, Cursor, VS Code, Codex,
  Gemini, Goose, Copilot, and OpenCode.

Example:
    ```python
    from pathlib import Path
    from mcpstore.mcp import MCPStore
    from mcpstore.mcp.server.providers.skills import ClaudeSkillsProvider, SkillProvider

    mcp = MCPStore("Skills Server")

    # Load a single skill
    mcp.add_provider(SkillProvider(Path.home() / ".claude/skills/pdf-processing"))

    # Or load all skills in a directory
    mcp.add_provider(ClaudeSkillsProvider())  # Uses ~/.claude/skills/
    ```
"""

from __future__ import annotations

# Import providers
from mcpstore.mcp.server.providers.skills.claude_provider import ClaudeSkillsProvider
from mcpstore.mcp.server.providers.skills.directory_provider import SkillsDirectoryProvider
from mcpstore.mcp.server.providers.skills.skill_provider import SkillProvider
from mcpstore.mcp.server.providers.skills.vendor_providers import (
    CodexSkillsProvider,
    CopilotSkillsProvider,
    CursorSkillsProvider,
    GeminiSkillsProvider,
    GooseSkillsProvider,
    OpenCodeSkillsProvider,
    VSCodeSkillsProvider,
)

# Backwards compatibility alias
SkillsProvider = SkillsDirectoryProvider


__all__ = [
    "ClaudeSkillsProvider",
    "CodexSkillsProvider",
    "CopilotSkillsProvider",
    "CursorSkillsProvider",
    "GeminiSkillsProvider",
    "GooseSkillsProvider",
    "OpenCodeSkillsProvider",
    "SkillProvider",
    "SkillsDirectoryProvider",
    "SkillsProvider",  # Backwards compatibility alias
    "VSCodeSkillsProvider",
]
