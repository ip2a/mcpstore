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
    "OpenKeyvMemoryConfig",

    # Utilities
    "PerspectiveResolver",

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
