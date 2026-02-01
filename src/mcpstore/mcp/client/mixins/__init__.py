"""Client mixins for MCPStore."""

from mcpstore.mcp.client.mixins.prompts import ClientPromptsMixin
from mcpstore.mcp.client.mixins.resources import ClientResourcesMixin
from mcpstore.mcp.client.mixins.task_management import ClientTaskManagementMixin
from mcpstore.mcp.client.mixins.tools import ClientToolsMixin

__all__ = [
    "ClientPromptsMixin",
    "ClientResourcesMixin",
    "ClientTaskManagementMixin",
    "ClientToolsMixin",
]
