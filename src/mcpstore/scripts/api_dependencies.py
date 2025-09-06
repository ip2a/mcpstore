"""
MCPStore API Dependencies
依赖注入模块，用于解决循环导入问题
"""

from typing import Optional
from mcpstore import MCPStore

# 全局 store 实例
_global_store_instance: Optional[MCPStore] = None


def get_global_store() -> MCPStore:
    """获取全局 MCPStore 实例"""
    if _global_store_instance is None:
        raise RuntimeError("Global store instance not set. Call set_global_store() first.")
    return _global_store_instance


def set_global_store(store: MCPStore) -> None:
    """设置全局 MCPStore 实例"""
    global _global_store_instance
    _global_store_instance = store


def has_global_store() -> bool:
    """检查是否已设置全局 store 实例"""
    return _global_store_instance is not None


# 为了向后兼容，保留 get_store 函数名
get_store = get_global_store