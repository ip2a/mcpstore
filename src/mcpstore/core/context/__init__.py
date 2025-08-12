"""
MCPStore Context Package
Refactored context management module

This package splits the original large context.py file into multiple specialized modules:
- base_context: Core context class and basic functionality
- service_operations: Service-related operations
- tool_operations: Tool-related operations
- resources_prompts: Resources and Prompts functionality
- advanced_features: Advanced features
"""

from .types import ContextType
from .base_context import MCPStoreContext

__all__ = ['ContextType', 'MCPStoreContext']
