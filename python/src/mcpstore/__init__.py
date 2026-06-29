"""
MCPStore - Model Context Protocol Service Management SDK
A composable, ready-to-use MCP toolkit for AI Agents and rapid integration.
"""

__version__ = "1.5.18"


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

    if name in ("MCPStoreContext", "Session", "SessionContext"):
        from mcpstore.core.context import MCPStoreContext, Session, SessionContext

        globals().update({
            "MCPStoreContext": MCPStoreContext,
            "Session": Session,
            "SessionContext": SessionContext,
        })
        return globals()[name]

    # Cache config classes
    if name in ("MemoryConfig", "RedisConfig", "OpenKeyvMemoryConfig"):
        from mcpstore.config import (
            MemoryConfig,
            OpenKeyvMemoryConfig,
            RedisConfig,
        )

        globals().update({
            "MemoryConfig": MemoryConfig,
            "RedisConfig": RedisConfig,
            "OpenKeyvMemoryConfig": OpenKeyvMemoryConfig,
        })
        return globals()[name]

    if name == "PerspectiveResolver":
        from mcpstore._rust import PerspectiveResolver

        globals()["PerspectiveResolver"] = PerspectiveResolver
        return PerspectiveResolver

    # Compatibility models and helpers for the Python SDK surface. Store
    # behavior remains delegated to the Rust/PyO3 core.
    models = {
        "APIResponse",
        "ClientIDGenerator",
        "ErrorCode",
        "ErrorDetail",
        "MCPStoreException",
        "Pagination",
        "ResponseBuilder",
        "ResponseMeta",
        "ServiceConnectionState",
        "ServiceInfo",
        "ToolExecutionError",
        "ToolExecutionRequest",
        "ToolInfo",
        "ServiceNotFoundException",
        "ValidationException",
        "timed_response",
    }
    if name in models:
        from mcpstore.core import models as core_models

        value = getattr(core_models, name)
        globals()[name] = value
        return value

    # Adapter common utilities
    if name in ("call_tool_response_helper", "ToolCallView"):
        from mcpstore.adapters.common import call_tool_response_helper, ToolCallView

        globals().update({
            "call_tool_response_helper": call_tool_response_helper,
            "ToolCallView": ToolCallView,
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
        module = __import__(f"mcpstore.adapters.{module_name}", fromlist=[name])
        adapter_class = getattr(module, name)

        globals()[name] = adapter_class
        return adapter_class

    raise AttributeError(f"module '{__name__}' has no attribute '{name}'")


# ===== Public Exports (API surface) =====
__all__ = [
    # Core Classes
    "MCPStore",
    "MCPStoreContext",
    "Session",
    "SessionContext",
    "LoggingConfig",

    # Cache Config
    "MemoryConfig",
    "RedisConfig",
    "OpenKeyvMemoryConfig",

    # Utilities
    "PerspectiveResolver",
    "ServiceInfo",
    "ServiceConnectionState",
    "ToolInfo",
    "ToolExecutionRequest",
    "APIResponse",
    "ErrorDetail",
    "ResponseMeta",
    "Pagination",
    "ResponseBuilder",
    "timed_response",
    "ErrorCode",
    "ClientIDGenerator",
    "MCPStoreException",
    "ServiceNotFoundException",
    "ToolExecutionError",
    "ValidationException",

    # Adapter Utilities
    "call_tool_response_helper",
    "ToolCallView",

    # Adapters
    "LangChainAdapter",
    "OpenAIAdapter",
    "AutoGenAdapter",
    "LlamaIndexAdapter",
    "CrewAIAdapter",
    "SemanticKernelAdapter",
]
