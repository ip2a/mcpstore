#!/usr/bin/env python3
"""
AsyncSyncHelper 已永久删除

此模块已被废弃并永久删除。根据 MCPStore 项目核心架构原则，
不再使用 AsyncSyncHelper 进行同步/异步桥接。

如果你看到这个错误，说明代码中仍在使用已废弃的 AsyncSyncHelper。
请按照下面的迁移指南修改代码。
"""

import logging

logger = logging.getLogger(__name__)

# 详细的错误信息和迁移指南
_DEPRECATION_MESSAGE = """
================================================================================
[错误] AsyncSyncHelper 已永久删除
================================================================================

你正在尝试使用已废弃的 AsyncSyncHelper。根据 MCPStore 项目核心架构原则，
此模块已被永久删除，不再支持。

--------------------------------------------------------------------------------
为什么删除？
--------------------------------------------------------------------------------
AsyncSyncHelper 使用后台线程和复杂的事件循环管理来桥接同步/异步代码，
这导致了：
  - 死锁风险
  - 调试困难
  - 代码复杂度高
  - 隐藏的性能问题

--------------------------------------------------------------------------------
新架构原则：双版本 API + 异步优先
--------------------------------------------------------------------------------

1. 异步方法 (_async) 包含所有业务逻辑
2. 同步方法只是对异步方法的薄包装，使用 asyncio.run() 转换
3. 只在最外层 API 做一次转换，禁止在中间层转换

--------------------------------------------------------------------------------
迁移指南
--------------------------------------------------------------------------------

【之前 - 错误的做法】:

    from mcpstore.core.utils.async_sync_helper import get_global_helper
    
    class SomeService:
        def __init__(self):
            self._sync_helper = get_global_helper()
        
        def list_items(self):
            return self._sync_helper.run_async(
                self.list_items_async(),
                timeout=30.0,
                force_background=True
            )

【之后 - 正确的做法】:

    import asyncio
    
    class SomeService:
        def list_items(self):
            \"\"\"同步版本 - 薄包装\"\"\"
            return asyncio.run(self.list_items_async())
        
        async def list_items_async(self):
            \"\"\"异步版本 - 核心实现\"\"\"
            return await self._kv_store.get(...)

--------------------------------------------------------------------------------
同步方法的使用限制
--------------------------------------------------------------------------------

同步方法只能在同步环境中调用，不能在异步环境中调用：

    # [正确] 在同步环境中调用同步方法
    def main():
        store = MCPStore.setup_store()
        tools = store.list_tools()  # OK

    # [正确] 在异步环境中调用异步方法
    async def main():
        store = MCPStore.setup_store()
        tools = await store.list_tools_async()  # OK

    # [错误] 在异步环境中调用同步方法
    async def main():
        tools = store.list_tools()  # 会报错

--------------------------------------------------------------------------------
详细文档
--------------------------------------------------------------------------------

请参阅: .kiro/steering/tech.md

================================================================================
"""


class AsyncSyncHelperRemoved(Exception):
    """AsyncSyncHelper 已被永久删除的异常"""
    pass


def _raise_removed_error(func_name: str):
    """抛出已删除错误"""
    logger.error(_DEPRECATION_MESSAGE)
    raise AsyncSyncHelperRemoved(
        f"\n{_DEPRECATION_MESSAGE}\n"
        f"触发位置: {func_name}()\n"
    )


class AsyncSyncHelper:
    """
    [已删除] AsyncSyncHelper 已永久删除
    
    此类的所有方法都会抛出 AsyncSyncHelperRemoved 异常。
    请按照错误信息中的迁移指南修改代码。
    """
    
    def __init__(self):
        _raise_removed_error("AsyncSyncHelper.__init__")
    
    def run_async(self, *args, **kwargs):
        _raise_removed_error("AsyncSyncHelper.run_async")
    
    def sync_wrapper(self, *args, **kwargs):
        _raise_removed_error("AsyncSyncHelper.sync_wrapper")
    
    def cleanup(self):
        _raise_removed_error("AsyncSyncHelper.cleanup")
    
    def _ensure_loop(self):
        _raise_removed_error("AsyncSyncHelper._ensure_loop")
    
    def _create_background_loop(self):
        _raise_removed_error("AsyncSyncHelper._create_background_loop")


def get_global_helper():
    """
    [已删除] get_global_helper 已永久删除
    
    此函数会抛出 AsyncSyncHelperRemoved 异常。
    请按照错误信息中的迁移指南修改代码。
    """
    _raise_removed_error("get_global_helper")


def run_async_sync(*args, **kwargs):
    """
    [已删除] run_async_sync 已永久删除
    
    此函数会抛出 AsyncSyncHelperRemoved 异常。
    请按照错误信息中的迁移指南修改代码。
    """
    _raise_removed_error("run_async_sync")


def async_to_sync(*args, **kwargs):
    """
    [已删除] async_to_sync 已永久删除
    
    此函数会抛出 AsyncSyncHelperRemoved 异常。
    请按照错误信息中的迁移指南修改代码。
    """
    _raise_removed_error("async_to_sync")


def cleanup_global_helper():
    """
    [已删除] cleanup_global_helper 已永久删除
    
    此函数会抛出 AsyncSyncHelperRemoved 异常。
    请按照错误信息中的迁移指南修改代码。
    """
    _raise_removed_error("cleanup_global_helper")


# 保留导出，但所有调用都会抛出错误
__all__ = [
    'AsyncSyncHelper',
    'AsyncSyncHelperRemoved',
    'get_global_helper',
    'run_async_sync',
    'async_to_sync',
    'cleanup_global_helper',
]
