#!/usr/bin/env python3
"""
MCPStore 同步/异步双向兼容设计方案
"""

import asyncio
import functools
from typing import Any, Callable, TypeVar, Union
from concurrent.futures import ThreadPoolExecutor
import threading

F = TypeVar('F', bound=Callable[..., Any])

class AsyncSyncMixin:
    """异步/同步双向兼容混入类"""
    
    def __init__(self):
        self._executor = ThreadPoolExecutor(max_workers=4, thread_name_prefix="mcpstore_sync")
        self._loop = None
        self._loop_thread = None
    
    def _get_or_create_loop(self):
        """获取或创建事件循环"""
        if self._loop is None or self._loop.is_closed():
            # 创建新的事件循环在独立线程中运行
            def run_loop():
                self._loop = asyncio.new_event_loop()
                asyncio.set_event_loop(self._loop)
                self._loop.run_forever()
            
            self._loop_thread = threading.Thread(target=run_loop, daemon=True)
            self._loop_thread.start()
            
            # 等待循环启动
            while self._loop is None:
                threading.Event().wait(0.01)
        
        return self._loop
    
    def _run_async_in_sync(self, coro):
        """在同步环境中运行异步函数"""
        try:
            # 尝试获取当前事件循环
            current_loop = asyncio.get_running_loop()
            # 如果已经在事件循环中，使用线程池执行
            future = asyncio.run_coroutine_threadsafe(coro, self._get_or_create_loop())
            return future.result(timeout=30)  # 30秒超时
        except RuntimeError:
            # 没有运行中的事件循环，直接运行
            return asyncio.run(coro)
    
    def sync_wrapper(self, async_func: F) -> F:
        """将异步函数包装为同步函数"""
        @functools.wraps(async_func)
        def wrapper(*args, **kwargs):
            coro = async_func(*args, **kwargs)
            return self._run_async_in_sync(coro)
        return wrapper

# 方案A：为每个方法提供同步和异步版本
class MCPStoreContextDualAPI(AsyncSyncMixin):
    """双API版本的MCPStoreContext"""
    
    def __init__(self, store, agent_id=None):
        super().__init__()
        self._store = store
        self._agent_id = agent_id
    
    # ==================== 异步版本（原有） ====================
    
    async def list_services_async(self):
        """异步获取服务列表"""
        # 原有的异步实现
        return await self._store.list_services(self._agent_id)
    
    async def add_service_async(self, config):
        """异步添加服务"""
        # 原有的异步实现
        return await self._store.add_service_impl(config, self._agent_id)
    
    async def use_tool_async(self, tool_name: str, args: dict):
        """异步使用工具"""
        # 原有的异步实现
        return await self._store.use_tool_impl(tool_name, args, self._agent_id)
    
    # ==================== 同步版本（新增） ====================
    
    def list_services(self):
        """同步获取服务列表"""
        return self.sync_wrapper(self.list_services_async)()
    
    def add_service(self, config):
        """同步添加服务"""
        return self.sync_wrapper(self.add_service_async)(config)
    
    def use_tool(self, tool_name: str, args: dict):
        """同步使用工具"""
        return self.sync_wrapper(self.use_tool_async)(tool_name, args)
    
    # ==================== 本来就是同步的方法 ====================
    
    def show_mcpconfig(self):
        """显示MCP配置（本来就是同步）"""
        return self._store.config.load_config()
    
    def reset_config(self):
        """重置配置（本来就是同步）"""
        return self._store.config.reset_config()

# 方案B：使用装饰器自动生成同步版本
def dual_api(async_func):
    """装饰器：自动为异步方法生成同步版本"""
    def decorator(cls):
        # 获取异步方法名
        async_name = async_func.__name__
        sync_name = async_name.replace('_async', '') if async_name.endswith('_async') else async_name
        
        # 创建同步版本
        def sync_method(self, *args, **kwargs):
            coro = async_func(self, *args, **kwargs)
            return self._run_async_in_sync(coro)
        
        sync_method.__name__ = sync_name
        sync_method.__doc__ = f"同步版本的 {async_name}"
        
        # 添加到类中
        setattr(cls, sync_name, sync_method)
        return cls
    
    return decorator

# 方案C：智能方法调度
class SmartMethodDispatcher:
    """智能方法调度器"""
    
    def __init__(self, async_method, sync_wrapper_func):
        self.async_method = async_method
        self.sync_wrapper = sync_wrapper_func
        self.__name__ = async_method.__name__
        self.__doc__ = async_method.__doc__
    
    def __call__(self, *args, **kwargs):
        """根据调用环境自动选择同步或异步执行"""
        try:
            # 检查是否在异步环境中
            asyncio.get_running_loop()
            # 在异步环境中，返回协程
            return self.async_method(*args, **kwargs)
        except RuntimeError:
            # 在同步环境中，执行同步版本
            coro = self.async_method(*args, **kwargs)
            return self.sync_wrapper(coro)
    
    def __await__(self):
        """支持await调用"""
        return self.async_method(*args, **kwargs).__await__()

# 使用示例
class ExampleUsage:
    """使用示例"""
    
    def basic_sync_usage(self):
        """基础同步用法"""
        store = MCPStore.setup_store()
        
        # 简单的同步调用
        services = store.for_store().list_services()
        tools = store.for_store().list_tools()
        
        # 添加服务
        store.for_store().add_service({
            "name": "weather",
            "url": "http://weather.example.com/mcp"
        })
        
        # 使用工具
        result = store.for_store().use_tool("weather_get_current", {"city": "北京"})
        
        return services, tools, result
    
    async def advanced_async_usage(self):
        """高级异步用法"""
        store = MCPStore.setup_store()
        
        # 并发执行多个操作
        services_task = store.for_store().list_services_async()
        tools_task = store.for_store().list_tools_async()
        
        services, tools = await asyncio.gather(services_task, tools_task)
        
        # 批量添加服务
        add_tasks = [
            store.for_store().add_service_async({"name": "weather", "url": "..."}),
            store.for_store().add_service_async({"name": "news", "url": "..."})
        ]
        
        await asyncio.gather(*add_tasks)
        
        return services, tools

# 推荐的最终API设计
class RecommendedMCPStoreContext:
    """推荐的MCPStoreContext设计"""
    
    def __init__(self, store, agent_id=None):
        self._store = store
        self._agent_id = agent_id
        self._sync_helper = AsyncSyncMixin()
    
    # ==================== 主要API（同步，用户友好） ====================
    
    def list_services(self):
        """获取服务列表（同步）"""
        return self._sync_helper._run_async_in_sync(self._list_services_impl())
    
    def add_service(self, config):
        """添加服务（同步）"""
        return self._sync_helper._run_async_in_sync(self._add_service_impl(config))
    
    def use_tool(self, tool_name: str, args: dict):
        """使用工具（同步）"""
        return self._sync_helper._run_async_in_sync(self._use_tool_impl(tool_name, args))
    
    # ==================== 异步版本（高级用户） ====================
    
    async def list_services_async(self):
        """获取服务列表（异步）"""
        return await self._list_services_impl()
    
    async def add_service_async(self, config):
        """添加服务（异步）"""
        return await self._add_service_impl(config)
    
    async def use_tool_async(self, tool_name: str, args: dict):
        """使用工具（异步）"""
        return await self._use_tool_impl(tool_name, args)
    
    # ==================== 内部实现（异步） ====================
    
    async def _list_services_impl(self):
        """内部异步实现"""
        # 实际的异步逻辑
        pass
    
    async def _add_service_impl(self, config):
        """内部异步实现"""
        # 实际的异步逻辑
        pass
    
    async def _use_tool_impl(self, tool_name: str, args: dict):
        """内部异步实现"""
        # 实际的异步逻辑
        pass
    
    # ==================== 本来就是同步的方法 ====================
    
    def show_mcpconfig(self):
        """显示MCP配置"""
        return self._store.config.load_config()
    
    def reset_config(self):
        """重置配置"""
        return self._store.config.reset_config()

if __name__ == "__main__":
    # 演示用法
    print("=== MCPStore 双向兼容API设计 ===")
    
    # 同步用法（推荐给普通用户）
    print("\n1. 同步用法（简单）:")
    print("""
    store = MCPStore.setup_store()
    services = store.for_store().list_services()  # 同步调用
    store.for_store().add_service(config)         # 同步调用
    result = store.for_store().use_tool(name, args)  # 同步调用
    """)
    
    # 异步用法（推荐给高级用户）
    print("\n2. 异步用法（高性能）:")
    print("""
    async def main():
        store = MCPStore.setup_store()
        services = await store.for_store().list_services_async()  # 异步调用
        await store.for_store().add_service_async(config)         # 异步调用
        result = await store.for_store().use_tool_async(name, args)  # 异步调用
    """)
    
    print("\n3. 优势:")
    print("   ✅ 用户友好：默认同步API，简单易用")
    print("   ✅ 性能优化：提供异步API，支持并发")
    print("   ✅ 向后兼容：不破坏现有代码")
    print("   ✅ 渐进式：用户可以按需选择同步或异步")
