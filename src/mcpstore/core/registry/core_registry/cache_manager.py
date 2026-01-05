"""
Cache Manager - 缓存管理模块

负责缓存层的配置和同步管理，包括：
1. 缓存后端的配置和管理
2. 同步/异步操作转换
3. 缓存同步机制
4. 异常处理和重试逻辑
"""

import asyncio
import logging
from typing import Dict, Any, Optional, Callable, List

from .base import CacheManagerInterface
from .errors import raise_legacy_error
from ...bridge import get_async_bridge

logger = logging.getLogger(__name__)


class CacheManager(CacheManagerInterface):
    """
    缓存管理器实现

    职责：
    - 管理缓存后端配置
    - 处理同步到异步的转换
    - 提供缓存同步机制
    - 异常处理和重试
    """

    def __init__(self, cache_layer, naming_service, namespace: str = "default"):
        super().__init__(cache_layer, naming_service, namespace)

        # 缓存后端配置
        self._cache_backend = None

        self._bridge = get_async_bridge()

        # 缓存同步状态
        self._sync_status = {}

        # 重试配置
        self._retry_config = {
            "max_retries": 3,
            "retry_delay": 1.0,
            "backoff_factor": 2.0
        }

        self._logger.info(f"Initializing CacheManager, namespace: {namespace}")

    def _legacy(self, method: str) -> None:
        raise_legacy_error(
            f"core_registry.CacheManager.{method}",
            "Use CacheLayerManager and domain shells for cache operations.",
        )

    def initialize(self) -> None:
        """初始化缓存管理器"""
        self._logger.info("CacheManager initialization completed")

    def cleanup(self) -> None:
        """清理缓存管理器资源"""
        try:
            # 清理缓存后端
            if self._cache_backend:
                try:
                    if hasattr(self._cache_backend, 'close'):
                        self._cache_backend.close()
                except Exception as e:
                    self._logger.warning(f"Error closing cache backend: {e}")

            # 清理同步助手
            self._sync_helper = None

            # 清理同步状态
            self._sync_status.clear()

            self._logger.info("CacheManager cleanup completed")
        except Exception as e:
            self._logger.error(f"CacheManager cleanup error: {e}")
            raise

    def configure_cache_backend(self, cache_config: Dict[str, Any]) -> None:
        """
        配置缓存后端

        Args:
            cache_config: 缓存配置字典
        """
        self._legacy("configure_cache_backend")

    def _create_cache_backend(self, cache_config: Dict[str, Any]):
        """
        创建缓存后端实例

        Args:
            cache_config: 缓存配置

        Returns:
            缓存后端实例
        """
        self._legacy("_create_cache_backend")

    def _create_memory_backend(self, config: Dict[str, Any]):
        """创建内存缓存后端"""
        self._legacy("_create_memory_backend")

    def _create_redis_backend(self, config: Dict[str, Any]):
        """创建Redis缓存后端"""
        self._legacy("_create_redis_backend")

    def _create_file_backend(self, config: Dict[str, Any]):
        """创建文件缓存后端"""
        self._legacy("_create_file_backend")

    def cleanup_cache_backend(self):
        """清理现有的缓存后端"""
        self._legacy("cleanup_cache_backend")

    def ensure_sync_helper(self):
        """
        向后兼容的同步助手接口，实际返回异步桥实例。
        """
        self._legacy("ensure_sync_helper")

    def sync_to_storage(self, operation_name: str = "缓存同步") -> Any:
        """
        同步到存储（同步方法调用异步操作）

        Args:
            operation_name: 操作名称，用于日志记录

        Returns:
            异步操作的结果
        """
        self._legacy("sync_to_storage")

    def async_to_sync(self, async_coro, operation_name: str = "异步转同步") -> Any:
        """
        将异步协程转换为同步调用

        Args:
            async_coro: 异步协程
            operation_name: 操作名称

        Returns:
            异步协程的结果
        """
        self._legacy("async_to_sync")

    def retry_operation(self, operation: Callable, *args, **kwargs) -> Any:
        """
        带重试的操作执行

        Args:
            operation: 要执行的操作函数
            *args: 位置参数
            **kwargs: 关键字参数

        Returns:
            操作结果
        """
        self._legacy("retry_operation")

    def get_sync_status(self, operation_name: Optional[str] = None) -> Dict[str, Any]:
        """
        获取同步状态信息

        Args:
            operation_name: 可选的操作名称，如果为None则返回所有状态

        Returns:
            同步状态信息
        """
        self._legacy("get_sync_status")

    def clear_sync_status(self, operation_name: Optional[str] = None):
        """
        清理同步状态

        Args:
            operation_name: 可选的操作名称，如果为None则清理所有状态
        """
        self._legacy("clear_sync_status")

    def get_backend_info(self) -> Dict[str, Any]:
        """
        获取缓存后端信息

        Returns:
            后端信息字典
        """
        self._legacy("get_backend_info")

    def get_stats(self) -> Dict[str, Any]:
        """
        获取缓存管理器的统计信息

        Returns:
            统计信息字典
        """
        self._legacy("get_stats")


class AsyncSyncHelper:
    """异步同步助手，用于在同步环境中运行异步操作"""

    def __init__(self):
        self._loop = None

    def run_sync(self, coro):
        """
        在同步环境中运行异步协程

        Args:
            coro: 异步协程

        Returns:
            异步操作的结果
        """
        try:
            # 尝试获取当前事件循环
            loop = asyncio.get_event_loop()
            if loop.is_running():
                # 如果事件循环正在运行，我们需要在新线程中运行
                import concurrent.futures
                import threading

                def run_in_thread():
                    new_loop = asyncio.new_event_loop()
                    asyncio.set_event_loop(new_loop)
                    try:
                        return new_loop.run_until_complete(coro)
                    finally:
                        new_loop.close()

                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(run_in_thread)
                    return future.result()
            else:
                # 如果事件循环没有运行，直接运行
                return loop.run_until_complete(coro)
        except RuntimeError:
            # 没有事件循环，创建一个新的
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            try:
                return loop.run_until_complete(coro)
            finally:
                loop.close()


# 简单内存缓存后端（作为后备方案）
class SimpleMemoryBackend:
    """简单的内存缓存后端实现，作为 kv_store_factory 失败时的后备方案"""

    def __init__(self, config):
        self.type = "memory"
        self.config = config
        self._data = {}
        self._stats = {
            "hits": 0,
            "misses": 0,
            "sets": 0,
            "deletes": 0
        }
        self._max_size = config.get("max_size", 10000)
        self._logger = logging.getLogger(self.__class__.__name__)

    def get(self, key: str) -> Optional[str]:
        """获取缓存值"""
        if key in self._data:
            self._stats["hits"] += 1
            return self._data[key]
        else:
            self._stats["misses"] += 1
            return None

    def set(self, key: str, value: str, ttl: Optional[int] = None) -> bool:
        """设置缓存值"""
        try:
            # 如果超过最大大小，执行简单的LRU清理
            if len(self._data) >= self._max_size:
                # 简单策略：删除一半的条目
                keys_to_remove = list(self._data.keys())[:self._max_size // 2]
                for k in keys_to_remove:
                    del self._data[k]

            self._data[key] = value
            self._stats["sets"] += 1
            return True
        except Exception as e:
            self._logger.error(f"Failed to set cache: {e}")
            return False

    def delete(self, key: str) -> bool:
        """删除缓存值"""
        try:
            if key in self._data:
                del self._data[key]
                self._stats["deletes"] += 1
                return True
            return False
        except Exception as e:
            self._logger.error(f"Failed to delete cache: {e}")
            return False

    def clear(self) -> bool:
        """清空缓存"""
        try:
            self._data.clear()
            self._stats = {
                "hits": 0,
                "misses": 0,
                "sets": 0,
                "deletes": 0
            }
            return True
        except Exception as e:
            self._logger.error(f"Failed to clear cache: {e}")
            return False

    def exists(self, key: str) -> bool:
        """检查键是否存在"""
        return key in self._data

    def keys(self, pattern: str = "*") -> List[str]:
        """获取匹配模式的键列表"""
        import fnmatch
        return [key for key in self._data.keys() if fnmatch.fnmatch(key, pattern)]

    def get_info(self) -> Dict[str, Any]:
        """获取缓存后端信息"""
        return {
            "type": "memory",
            "items_count": len(self._data),
            "max_size": self._max_size,
            "stats": self._stats.copy(),
            "memory_usage": sum(len(k) + len(str(v)) for k, v in self._data.items())
        }

    def cleanup(self):
        """清理缓存后端"""
        self.clear()
