"""Compatibility exports for the historical ``mcpstore.core.context`` package.

The public objects here are aliases to the Rust-backed Python facade. Session
state and session lifecycle behavior remain owned by Rust core.
"""

from .agent_service_mapper import AgentServiceMapper
from .agent_statistics import AgentStatisticsMixin
from .agent_proxy import AgentProxy
from .advanced_features import AdvancedFeaturesMixin
from .async_safe_service_management import (
    AsyncSafeServiceManagement,
    AsyncSafeServiceManagementFactory,
)
from .base_context import MCPStoreContext
from .cache_proxy import CacheProxy
from .resources_prompts import ResourcesPromptsMixin
from .service_operations import AddServiceWaitStrategy, ServiceOperationsMixin
from .service_management import UpdateServiceAuthHelper
from .service_proxy import ServiceProxy
from .session import Session, SessionContext
from .session_management import SessionManagementMixin
from .tool_proxy import ToolCallResult, ToolProxy
from .tool_operations import ToolOperationsMixin
from .tool_proxy_annotations import CallToolResultProtocol
from .tool_transformation import (
    ArgumentTransform,
    ToolTransformConfig,
    ToolTransformationManager,
    ToolTransformer,
    TransformationType,
    get_transformation_manager,
)
from .store_proxy import StoreProxy
from .types import ContextType

__all__ = [
    "MCPStoreContext",
    "StoreProxy",
    "AgentProxy",
    "ServiceProxy",
    "ToolProxy",
    "ToolCallResult",
    "AgentServiceMapper",
    "AgentStatisticsMixin",
    "AdvancedFeaturesMixin",
    "AsyncSafeServiceManagement",
    "AsyncSafeServiceManagementFactory",
    "UpdateServiceAuthHelper",
    "AddServiceWaitStrategy",
    "ServiceOperationsMixin",
    "ToolOperationsMixin",
    "SessionManagementMixin",
    "CallToolResultProtocol",
    "ContextType",
    "CacheProxy",
    "ResourcesPromptsMixin",
    "Session",
    "SessionContext",
    "ArgumentTransform",
    "ToolTransformConfig",
    "ToolTransformationManager",
    "ToolTransformer",
    "TransformationType",
    "get_transformation_manager",
]
