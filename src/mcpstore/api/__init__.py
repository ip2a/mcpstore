"""
对外稳定接口层：集中导出核心能力，方便后续内部目录调整时保持兼容。
当前仅简单导出 orchestrator，后续可逐步补充配置加载、CLI 包装等入口。
"""

from mcpstore.core.orchestrator import MCPOrchestrator

__all__ = ["MCPOrchestrator"]
