"""
逻辑核心模块

包含所有纯同步的业务逻辑，不包含任何 IO 操作。
遵循 Functional Core, Imperative Shell 架构原则。
"""

from .tool_logic import ToolLogicCore

__all__ = ["ToolLogicCore"]
