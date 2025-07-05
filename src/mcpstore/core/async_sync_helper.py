#!/usr/bin/env python3
"""
异步/同步兼容助手
提供在同步环境中运行异步函数的能力
"""

import asyncio
import threading
import functools
from typing import Any, Coroutine, TypeVar
from concurrent.futures import ThreadPoolExecutor
import logging

# 确保logger始终可用
try:
    logger = logging.getLogger(__name__)
except Exception:
    # 如果出现任何问题，创建一个基本的logger
    import sys
    logger = logging.getLogger(__name__)
    if not logger.handlers:
        handler = logging.StreamHandler(sys.stderr)
        handler.setFormatter(logging.Formatter('%(name)s - %(levelname)s - %(message)s'))
        logger.addHandler(handler)
        logger.setLevel(logging.INFO)

T = TypeVar('T')

class AsyncSyncHelper:
    """异步/同步兼容助手类"""
    
    def __init__(self):
        self._executor = ThreadPoolExecutor(
            max_workers=4, 
            thread_name_prefix="mcpstore_sync"
        )
        self._loop = None
        self._loop_thread = None
        self._lock = threading.Lock()
    
    def _ensure_loop(self):
        """确保事件循环存在并运行"""
        if self._loop is None or self._loop.is_closed():
            with self._lock:
                # 双重检查锁定
                if self._loop is None or self._loop.is_closed():
                    self._create_background_loop()
        return self._loop
    
    def _create_background_loop(self):
        """在后台线程中创建事件循环"""
        loop_ready = threading.Event()
        
        def run_loop():
            """在独立线程中运行事件循环"""
            try:
                self._loop = asyncio.new_event_loop()
                asyncio.set_event_loop(self._loop)
                loop_ready.set()
                logger.debug("Background event loop started")
                self._loop.run_forever()
            except Exception as e:
                logger.error(f"Background loop error: {e}")
            finally:
                logger.debug("Background event loop stopped")
        
        self._loop_thread = threading.Thread(
            target=run_loop, 
            daemon=True,
            name="mcpstore_event_loop"
        )
        self._loop_thread.start()
        
        # 等待循环启动
        if not loop_ready.wait(timeout=5):
            raise RuntimeError("Failed to start background event loop")
    
    def run_async(self, coro: Coroutine[Any, Any, T], timeout: float = 30.0) -> T:
        """
        在同步环境中运行异步函数
        
        Args:
            coro: 协程对象
            timeout: 超时时间（秒）
            
        Returns:
            协程的执行结果
            
        Raises:
            TimeoutError: 执行超时
            RuntimeError: 执行失败
        """
        try:
            # 检查是否已经在事件循环中
            try:
                current_loop = asyncio.get_running_loop()
                # 如果已经在事件循环中，使用后台循环
                logger.debug("Running coroutine in background loop (nested)")
                loop = self._ensure_loop()
                future = asyncio.run_coroutine_threadsafe(coro, loop)
                return future.result(timeout=timeout)
            except RuntimeError:
                # 没有运行中的事件循环，使用 asyncio.run
                logger.debug("Running coroutine with asyncio.run")
                return asyncio.run(coro)

        except Exception as e:
            logger.error(f"Error running async function: {e}")
            raise
    
    def sync_wrapper(self, async_func):
        """
        将异步函数包装为同步函数的装饰器
        
        Args:
            async_func: 异步函数
            
        Returns:
            同步版本的函数
        """
        @functools.wraps(async_func)
        def wrapper(*args, **kwargs):
            coro = async_func(*args, **kwargs)
            return self.run_async(coro)
        
        return wrapper
    
    def cleanup(self):
        """清理资源"""
        try:
            if self._loop and not self._loop.is_closed():
                # 停止事件循环
                self._loop.call_soon_threadsafe(self._loop.stop)
                
            if self._loop_thread and self._loop_thread.is_alive():
                # 等待线程结束
                self._loop_thread.join(timeout=2)
                
            if self._executor:
                # 关闭线程池（Python 3.9+才支持timeout参数）
                try:
                    self._executor.shutdown(wait=True, timeout=2)
                except TypeError:
                    # 兼容旧版本Python
                    self._executor.shutdown(wait=True)
                
            logger.debug("AsyncSyncHelper cleanup completed")
            
        except Exception as e:
            logger.error(f"Error during cleanup: {e}")
    
    def __del__(self):
        """析构函数，确保资源清理"""
        try:
            self.cleanup()
        except:
            pass  # 忽略析构时的错误


# 全局实例，用于整个MCPStore
_global_helper = None
_helper_lock = threading.Lock()

def get_global_helper() -> AsyncSyncHelper:
    """获取全局的AsyncSyncHelper实例"""
    global _global_helper
    
    if _global_helper is None:
        with _helper_lock:
            if _global_helper is None:
                _global_helper = AsyncSyncHelper()
    
    return _global_helper

def run_async_sync(coro: Coroutine[Any, Any, T], timeout: float = 30.0) -> T:
    """
    便捷函数：在同步环境中运行异步函数
    
    Args:
        coro: 协程对象
        timeout: 超时时间（秒）
        
    Returns:
        协程的执行结果
    """
    helper = get_global_helper()
    return helper.run_async(coro, timeout)

def async_to_sync(async_func):
    """
    装饰器：将异步函数转换为同步函数
    
    Usage:
        @async_to_sync
        async def my_async_func():
            return await some_async_operation()
        
        # 现在可以同步调用
        result = my_async_func()
    """
    @functools.wraps(async_func)
    def wrapper(*args, **kwargs):
        coro = async_func(*args, **kwargs)
        return run_async_sync(coro)
    
    return wrapper

# 清理函数，在程序退出时调用
def cleanup_global_helper():
    """清理全局helper资源"""
    global _global_helper
    
    if _global_helper:
        _global_helper.cleanup()
        _global_helper = None

# 注册清理函数
import atexit
atexit.register(cleanup_global_helper)

if __name__ == "__main__":
    # 测试代码
    import time
    
    async def test_async_func(delay: float, message: str):
        """测试异步函数"""
        await asyncio.sleep(delay)
        return f"Completed: {message}"
    
    def test_sync_usage():
        """测试同步用法"""
        print("Testing sync usage...")
        
        helper = AsyncSyncHelper()
        
        # 测试1: 基本异步调用
        result1 = helper.run_async(test_async_func(0.1, "test1"))
        print(f"Result 1: {result1}")
        
        # 测试2: 使用装饰器
        sync_func = helper.sync_wrapper(test_async_func)
        result2 = sync_func(0.1, "test2")
        print(f"Result 2: {result2}")
        
        # 测试3: 使用全局函数
        result3 = run_async_sync(test_async_func(0.1, "test3"))
        print(f"Result 3: {result3}")
        
        # 测试4: 使用装饰器
        @async_to_sync
        async def decorated_func():
            return await test_async_func(0.1, "decorated")
        
        result4 = decorated_func()
        print(f"Result 4: {result4}")
        
        helper.cleanup()
        print("Sync usage test completed")
    
    async def test_async_usage():
        """测试异步用法"""
        print("Testing async usage...")
        
        # 在异步环境中也应该能正常工作
        result = run_async_sync(test_async_func(0.1, "async_env"))
        print(f"Async env result: {result}")
        
        print("Async usage test completed")
    
    # 运行测试
    test_sync_usage()
    asyncio.run(test_async_usage())
    
    print("All tests completed")
