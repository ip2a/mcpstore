from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class KeyBuilder:
    """Key builder for Redis cache namespacing.

    Key layout:
      mcpstore:{namespace}:agent:{agent_id}:...
      mcpstore:{namespace}:client:{client_id}:...
      mcpstore:{namespace}:...

    The namespace provides isolation between different applications/environments.
    Default namespace is auto-generated from mcp.json path (5-char hash).
    """

    namespace: str = "mcpstore"

    def base(self) -> str:
        """Return base key prefix: mcpstore:{namespace}"""
        return f"mcpstore:{self.namespace}"

