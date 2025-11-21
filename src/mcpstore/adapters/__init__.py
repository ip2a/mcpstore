"""
Adapters module - Unified export of all adapters

Provides adapters for various AI frameworks, facilitating integration of MCPStore
into different AI Agent frameworks.
"""

# ===== Direct export of all adapters =====
from .langchain_adapter import LangChainAdapter
from .openai_adapter import OpenAIAdapter
from .autogen_adapter import AutoGenAdapter
from .llamaindex_adapter import LlamaIndexAdapter
from .crewai_adapter import CrewAIAdapter
from .semantic_kernel_adapter import SemanticKernelAdapter

# ===== Public exports =====
__all__ = [
    "LangChainAdapter",
    "OpenAIAdapter",
    "AutoGenAdapter",
    "LlamaIndexAdapter",
    "CrewAIAdapter",
    "SemanticKernelAdapter",
]
