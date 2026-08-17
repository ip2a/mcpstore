"""
MCPStore - Model Context Protocol Service Management SDK
A composable, ready-to-use MCP toolkit for AI Agents and rapid integration.
"""

__version__ = "2.0.2"


# ===== Lazy loading implementation =====
def __getattr__(name: str):
    """Lazy-load public objects on first access to reduce import overhead."""

    # Core classes
    if name in ("LoggingConfig", "MCPStore"):
        from mcpstore.config.config import LoggingConfig
        from mcpstore.store import MCPStore

        globals().update({
            "LoggingConfig": LoggingConfig,
            "MCPStore": MCPStore,
        })
        return globals()[name]

    if name in ("StoreContext", "AgentContext", "Service", "Tool"):
        from mcpstore.context import AgentContext, Service, StoreContext, Tool

        globals().update({
            "StoreContext": StoreContext,
            "AgentContext": AgentContext,
            "Service": Service,
            "Tool": Tool,
        })
        return globals()[name]

    if name == "SessionContext":
        from mcpstore.sessions import SessionContext

        globals()["SessionContext"] = SessionContext
        return SessionContext

    # Cache config classes
    if name in {
        "MemoryConfig", "StoreConfig", "FileConfig", "RedisConfig",
        "ValkeyConfig", "MemcachedConfig", "SqliteConfig", "PostgresConfig",
        "DuckDBConfig", "RocksDBConfig", "DiskConfig", "S3Config",
        "DynamoDBConfig", "MongoDBConfig", "FileTreeConfig",
    }:
        from mcpstore import config as _config

        value = getattr(_config, name)
        globals()[name] = value
        return value

    if name == "PerspectiveResolver":
        from mcpstore._rust import PerspectiveResolver

        globals()["PerspectiveResolver"] = PerspectiveResolver
        return PerspectiveResolver

    # Async Rust chain (native coroutine API)
    if name in {
        "AsyncMCPStore",
        "AsyncScopeContext",
        "AsyncService",
        "AsyncTool",
    }:
        from mcpstore._rust import (
            AsyncMCPStore,
            AsyncScopeContext,
            AsyncService,
            AsyncTool,
        )

        globals().update({
            "AsyncMCPStore": AsyncMCPStore,
            "AsyncScopeContext": AsyncScopeContext,
            "AsyncService": AsyncService,
            "AsyncTool": AsyncTool,
        })
        return globals()[name]

    # Public request models: scope references and service-config payloads.
    if name in {
        "CommandServiceConfig",
        "MCPServerConfig",
        "ServiceConfig",
        "ServiceConfigUnion",
        "ScopeDescriptor",
        "ScopeRef",
        "ScopeView",
        "RootScope",
        "StoreScope",
        "AgentScope",
        "URLServiceConfig",
    }:
        from mcpstore.core import models as core_models

        value = getattr(core_models, name)
        globals()[name] = value
        return value

    # Adapter common utilities
    if name in ("to_tool_call_view", "ToolCallView"):
        from mcpstore.adapters.common import to_tool_call_view, ToolCallView

        globals().update({
            "to_tool_call_view": to_tool_call_view,
            "ToolCallView": ToolCallView,
        })
        return globals()[name]

    # Adapter classes (lazy import, fall back to None if adapter is not installed)
    adapters_mapping = {
        "LangChainAdapter": "langchain_adapter",
        "SessionAwareLangChainAdapter": "langchain_adapter",
        "LangGraphAdapter": "langgraph_adapter",
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
    "StoreContext",
    "AgentContext",
    "SessionContext",
    "LoggingConfig",

    # Cache Config
    "MemoryConfig",
    "StoreConfig",
    "FileConfig",
    "RedisConfig",
    "ValkeyConfig",
    "MemcachedConfig",
    "SqliteConfig",
    "PostgresConfig",
    "DuckDBConfig",
    "RocksDBConfig",
    "DiskConfig",
    "S3Config",
    "DynamoDBConfig",
    "MongoDBConfig",
    "FileTreeConfig",

    # Utilities
    "PerspectiveResolver",

    # Async Rust chain (native coroutine API)
    "AsyncMCPStore",
    "AsyncScopeContext",
    "AsyncService",
    "AsyncTool",

    # Request Models (scope + service config)
    "ServiceConfig",
    "URLServiceConfig",
    "CommandServiceConfig",
    "MCPServerConfig",
    "ServiceConfigUnion",
    "RootScope",
    "StoreScope",
    "AgentScope",
    "ScopeRef",
    "ScopeView",
    "ScopeDescriptor",

    # Adapter Utilities
    "to_tool_call_view",
    "ToolCallView",

    # Adapters
    "LangChainAdapter",
    "SessionAwareLangChainAdapter",
    "LangGraphAdapter",
    "OpenAIAdapter",
    "AutoGenAdapter",
    "LlamaIndexAdapter",
    "CrewAIAdapter",
    "SemanticKernelAdapter",
]
