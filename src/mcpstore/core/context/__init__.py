"""
MCPStore Context Package
Refactored context management module

This package splits the original large context.py file into multiple specialized modules:
- base_context: Core context class and basic functionality
- service_operations: Service-related operations
- tool_operations: Tool-related operations
- resources_prompts: Resources and Prompts functionality
- advanced_features: Advanced features
- service_proxy: Service proxy object for specific service operations
"""

from .types import ContextType
from .base_context import MCPStoreContext
from .service_proxy import ServiceProxy

__all__ = ['ContextType', 'MCPStoreContext', 'ServiceProxy']
