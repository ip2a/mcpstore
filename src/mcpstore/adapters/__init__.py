"""
Adapters module - 所有适配器的统一导出

提供各种AI框架的适配器，方便将MCPStore集成到不同的AI Agent框架中。
"""

# ===== 所有适配器直接导出 =====
from .langchain_adapter import LangChainAdapter
from .openai_adapter import OpenAIAdapter
from .autogen_adapter import AutoGenAdapter
from .llamaindex_adapter import LlamaIndexAdapter
from .crewai_adapter import CrewAIAdapter
from .semantic_kernel_adapter import SemanticKernelAdapter

# ===== 公开所有导出 =====
__all__ = [
    "LangChainAdapter",
    "OpenAIAdapter",
    "AutoGenAdapter",
    "LlamaIndexAdapter",
    "CrewAIAdapter",
    "SemanticKernelAdapter",
]
