"""
MCPStore - Model Context Protocol Service Management SDK
A composable, ready-to-use MCP toolkit for AI Agents and rapid integration.
"""
 
__version__ = "1.5.13"


# ===== Lazy loading implementation =====
def __getattr__(name: str):
    """Lazy-load public objects on first access to reduce import overhead."""

    # Core classes
    if name in ("LoggingConfig", "MCPStore"):
        from mcpstore.config.config import LoggingConfig
        from mcpstore.core.store import MCPStore

        globals().update({
            "LoggingConfig": LoggingConfig,
            "MCPStore": MCPStore,
        })
        return globals()[name]

    # Cache config classes
    if name in ("MemoryConfig", "RedisConfig"):
        from mcpstore.config import MemoryConfig, RedisConfig

        globals().update({
            "MemoryConfig": MemoryConfig,
            "RedisConfig": RedisConfig,
        })
        return globals()[name]

    # Core model classes
    if name in ("ServiceInfo", "ServiceConnectionState", "ToolInfo", "ToolExecutionRequest"):
        from mcpstore.core.models.service import ServiceInfo, ServiceConnectionState
        from mcpstore.core.models.tool import ToolInfo, ToolExecutionRequest

        globals().update({
            "ServiceInfo": ServiceInfo,
            "ServiceConnectionState": ServiceConnectionState,
            "ToolInfo": ToolInfo,
            "ToolExecutionRequest": ToolExecutionRequest,
        })
        return globals()[name]

    if name in ("APIResponse", "ErrorDetail", "ResponseMeta", "Pagination", "ResponseBuilder"):
        from mcpstore.core.models.response import APIResponse, ErrorDetail, ResponseMeta, Pagination
        from mcpstore.core.models.response_builder import ResponseBuilder

        globals().update({
            "APIResponse": APIResponse,
            "ErrorDetail": ErrorDetail,
            "ResponseMeta": ResponseMeta,
            "Pagination": Pagination,
            "ResponseBuilder": ResponseBuilder,
        })
        return globals()[name]

    if name == "ErrorCode":
        from mcpstore.core.models.error_codes import ErrorCode

        globals()["ErrorCode"] = ErrorCode
        return ErrorCode

    # Core exception classes
    if name in ("MCPStoreException", "ServiceNotFoundException", "ToolExecutionError"):
        from mcpstore.core.exceptions import (
            MCPStoreException,
            ServiceNotFoundException,
            ToolExecutionError,
        )

        globals().update({
            "MCPStoreException": MCPStoreException,
            "ServiceNotFoundException": ServiceNotFoundException,
            "ToolExecutionError": ToolExecutionError,
        })
        return globals()[name]

    # Adapter classes (lazy import, fall back to None if adapter is not installed)
    adapters_mapping = {
        "LangChainAdapter": "langchain_adapter",
        "OpenAIAdapter": "openai_adapter",
        "AutoGenAdapter": "autogen_adapter",
        "LlamaIndexAdapter": "llamaindex_adapter",
        "CrewAIAdapter": "crewai_adapter",
        "SemanticKernelAdapter": "semantic_kernel_adapter",
    }

    if name in adapters_mapping:
        module_name = adapters_mapping[name]
        try:
            module = __import__(f"mcpstore.adapters.{module_name}", fromlist=[name])
            adapter_class = getattr(module, name)
        except ImportError:
            adapter_class = None

        globals()[name] = adapter_class
        return adapter_class

    raise AttributeError(f"module '{__name__}' has no attribute '{name}'")


# ===== Public Exports (API surface) =====
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
