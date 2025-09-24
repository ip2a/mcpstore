"""
MCPStore Context Package
Refactored context management module

This package splits the original large context.py file into multiple specialized modules:
- base_context: Core context class and basic functionality
- service_operations: Service-related operations
- tool_operations: Tool-related operations
- service_proxy: Service proxy object for specific service operations
- tool_proxy: Tool proxy object for specific tool operations
- tool_transformation: Tool transformation and enhancement functionality
- agent_service_mapper: Agent service name mapping functionality
- resources_prompts: Resources and Prompts functionality
- advanced_features: Advanced features
"""

from .types import ContextType
from .base_context import MCPStoreContext
from .service_proxy import ServiceProxy
from .tool_proxy import ToolProxy, ToolCallResult
from .agent_service_mapper import AgentServiceMapper
from .service_management import UpdateServiceAuthHelper
from .session import Session, SessionContext
from .session_management import SessionManagementMixin
from .tool_transformation import (
    ToolTransformer, 
    ToolTransformationManager, 
    ToolTransformConfig,
    ArgumentTransform,
    TransformationType,
    get_transformation_manager
)

__all__ = [
    'ContextType', 
    'MCPStoreContext', 
    'ServiceProxy', 
    'ToolProxy', 
    'ToolCallResult', 
    'AgentServiceMapper',
    'UpdateServiceAuthHelper',
    'Session',
    'SessionContext',
    'SessionManagementMixin',
    'ToolTransformer',
    'ToolTransformationManager', 
    'ToolTransformConfig',
    'ArgumentTransform',
    'TransformationType',
    'get_transformation_manager'
]
