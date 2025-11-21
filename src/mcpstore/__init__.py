"""
MCPStore - Model Context Protocol Service Management SDK
A composable, ready-to-use MCP toolkit for AI Agents and rapid integration.
"""

__version__ = "1.5.7"

# ===== Core Classes =====
from mcpstore.config.config import LoggingConfig
from mcpstore.core.store import MCPStore

# ===== Cache Config Classes =====
from mcpstore.config import MemoryConfig, RedisConfig

# ===== Core Model Classes =====
from mcpstore.core.models.service import ServiceInfo, ServiceConnectionState
from mcpstore.core.models.tool import ToolInfo, ToolExecutionRequest
from mcpstore.core.models.response import APIResponse, ErrorDetail, ResponseMeta, Pagination
from mcpstore.core.models.response_builder import ResponseBuilder
from mcpstore.core.models.error_codes import ErrorCode

# ===== Core Exception Classes =====
from mcpstore.core.exceptions import (
    MCPStoreException,
    ServiceNotFoundException,
    ToolExecutionError,
)

# ===== Adapter Classes (Direct Passthrough) =====
try:
    from mcpstore.adapters.langchain_adapter import LangChainAdapter
except ImportError:
    LangChainAdapter = None

try:
    from mcpstore.adapters.openai_adapter import OpenAIAdapter
except ImportError:
    OpenAIAdapter = None

try:
    from mcpstore.adapters.autogen_adapter import AutoGenAdapter
except ImportError:
    AutoGenAdapter = None

try:
    from mcpstore.adapters.llamaindex_adapter import LlamaIndexAdapter
except ImportError:
    LlamaIndexAdapter = None

try:
    from mcpstore.adapters.crewai_adapter import CrewAIAdapter
except ImportError:
    CrewAIAdapter = None

try:
    from mcpstore.adapters.semantic_kernel_adapter import SemanticKernelAdapter
except ImportError:
    SemanticKernelAdapter = None

# ===== Public Exports =====
__all__ = [
    # Core Classes
    "MCPStore",
    "LoggingConfig",

    # Cache Config
    "MemoryConfig",
    "RedisConfig",

    # Model Classes
    "ServiceInfo",
    "ServiceConnectionState",
    "ToolInfo",
    "ToolExecutionRequest",
    "APIResponse",
    "ResponseBuilder",
    "ErrorDetail",
    "ResponseMeta",
    "Pagination",
    "ErrorCode",

    # Exception Classes
    "MCPStoreException",
    "ServiceNotFoundException",
    "ToolExecutionError",

    # Adapters
    "LangChainAdapter",
    "OpenAIAdapter",
    "AutoGenAdapter",
    "LlamaIndexAdapter",
    "CrewAIAdapter",
    "SemanticKernelAdapter",
]
