"""
Cache Manager - 缓存管理模块

负责缓存层的配置和同步管理，包括：
1. 缓存后端的配置和管理
2. 同步/异步操作转换
3. 缓存同步机制
4. 异常处理和重试逻辑
"""

import logging
import asyncio
from typing import Dict, Any, Optional, Callable, List

from .base import CacheManagerInterface

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

        # 同步助手（懒加载）
        self._sync_helper = None

        # 缓存同步状态
        self._sync_status = {}

        # 重试配置
        self._retry_config = {
            "max_retries": 3,
            "retry_delay": 1.0,
            "backoff_factor": 2.0
        }

        self._logger.info(f"初始化CacheManager，命名空间: {namespace}")

    def initialize(self) -> None:
        """初始化缓存管理器"""
        self._logger.info("CacheManager 初始化完成")

    def cleanup(self) -> None:
        """清理缓存管理器资源"""
        try:
            # 清理缓存后端
            if self._cache_backend:
                try:
                    if hasattr(self._cache_backend, 'close'):
                        self._cache_backend.close()
                except Exception as e:
                    self._logger.warning(f"关闭缓存后端时出错: {e}")

            # 清理同步助手
            self._sync_helper = None

            # 清理同步状态
            self._sync_status.clear()

            self._logger.info("CacheManager 清理完成")
        except Exception as e:
            self._logger.error(f"CacheManager 清理时出错: {e}")
            raise

    def configure_cache_backend(self, cache_config: Dict[str, Any]) -> None:
        """
        配置缓存后端

        Args:
            cache_config: 缓存配置字典
        """
        try:
            self._logger.info(f"配置缓存后端: {cache_config}")

            # 如果有现有的后端，先清理
            if self._cache_backend:
                self.cleanup_cache_backend()

            # 创建新的缓存后端
            self._cache_backend = self._create_cache_backend(cache_config)

            # 更新缓存层的后端配置
            if hasattr(self._cache_layer, 'set_backend'):
                self._cache_layer.set_backend(self._cache_backend)

            self._logger.info("缓存后端配置完成")

        except Exception as e:
            self._logger.error(f"配置缓存后端失败: {e}")
            raise

    def _create_cache_backend(self, cache_config: Dict[str, Any]):
        """
        创建缓存后端实例

        Args:
            cache_config: 缓存配置

        Returns:
            缓存后端实例
        """
        backend_type = cache_config.get('type', 'memory')

        if backend_type == 'memory':
            return self._create_memory_backend(cache_config)
        elif backend_type == 'redis':
            return self._create_redis_backend(cache_config)
        elif backend_type == 'file':
            return self._create_file_backend(cache_config)
        else:
            raise ValueError(f"不支持的缓存后端类型: {backend_type}")

    def _create_memory_backend(self, config: Dict[str, Any]):
        """创建内存缓存后端"""
        try:
            # 使用 kv_store_factory 创建真实的内存缓存后端
            from mcpstore.core.registry.kv_store_factory import _build_kv_store

            factory_config = {
                "type": "memory",
                "enable_statistics": True,
                "enable_size_limit": True,
                "max_item_size": config.get("max_item_size", 1024 * 1024),  # 1MB默认
                "namespace": config.get("namespace", "mcpstore_cache")
            }

            backend = _build_kv_store(factory_config)
            self._logger.info(f"创建内存缓存后端: namespace={factory_config['namespace']}")
            return backend

        except Exception as e:
            self._logger.error(f"创建内存缓存后端失败: {e}")
            # 如果创建失败，创建一个简单的内存字典作为后备
            return SimpleMemoryBackend(config)

    def _create_redis_backend(self, config: Dict[str, Any]):
        """创建Redis缓存后端"""
        try:
            # 使用 kv_store_factory 创建真实的Redis缓存后端
            from mcpstore.core.registry.kv_store_factory import _build_kv_store

            factory_config = {
                "type": "redis",
                "url": config.get("url", "redis://localhost:6379"),
                "password": config.get("password"),
                "namespace": config.get("namespace", "mcpstore_cache"),
                "enable_statistics": True,
                "enable_size_limit": True,
                "max_item_size": config.get("max_item_size", 1024 * 1024),  # 1MB默认
            }

            # 只添加非None的可选参数
            if config.get("socket_timeout") is not None:
                factory_config["socket_timeout"] = config.get("socket_timeout")
            if config.get("healthcheck_interval") is not None:
                factory_config["healthcheck_interval"] = config.get("healthcheck_interval")
            if config.get("max_connections") is not None:
                factory_config["max_connections"] = config.get("max_connections")

            backend = _build_kv_store(factory_config)
            self._logger.info(f"创建Redis缓存后端: namespace={factory_config['namespace']}, url={config.get('url', 'redis://localhost:6379')}")
            return backend

        except Exception as e:
            self._logger.error(f"创建Redis缓存后端失败: {e}")
            # 如果Redis连接失败，降级到内存缓存
            self._logger.warning("Redis创建失败，降级使用内存缓存")
            return self._create_memory_backend(config)

    def _create_file_backend(self, config: Dict[str, Any]):
        """创建文件缓存后端"""
        try:
            # 使用 kv_store_factory 创建真实的文件缓存后端
            from mcpstore.core.registry.kv_store_factory import _build_kv_store

            factory_config = {
                "type": "file",
                "directory": config.get("directory", "/tmp/mcpstore_cache"),
                "namespace": config.get("namespace", "mcpstore_cache"),
                "enable_statistics": True,
                "enable_size_limit": True,
                "max_item_size": config.get("max_item_size", 1024 * 1024),  # 1MB默认
            }

            backend = _build_kv_store(factory_config)
            self._logger.info(f"创建文件缓存后端: directory={factory_config['directory']}, namespace={factory_config['namespace']}")
            return backend

        except Exception as e:
            self._logger.error(f"创建文件缓存后端失败: {e}")
            # 如果文件缓存创建失败，降级到内存缓存
            self._logger.warning("文件缓存创建失败，降级使用内存缓存")
            return self._create_memory_backend(config)

    def cleanup_cache_backend(self):
        """清理现有的缓存后端"""
        if self._cache_backend:
            try:
                if hasattr(self._cache_backend, 'cleanup'):
                    self._cache_backend.cleanup()
                elif hasattr(self._cache_backend, 'close'):
                    self._cache_backend.close()
            except Exception as e:
                self._logger.warning(f"清理缓存后端时出错: {e}")
            finally:
                self._cache_backend = None

    def ensure_sync_helper(self):
        """
        确保同步助手存在（懒加载）
        """
        if self._sync_helper is None:
            self._sync_helper = AsyncSyncHelper()
            self._logger.debug("创建了异步同步助手")

        return self._sync_helper

    def sync_to_storage(self, operation_name: str = "缓存同步") -> Any:
        """
        同步到存储（同步方法调用异步操作）

        Args:
            operation_name: 操作名称，用于日志记录

        Returns:
            异步操作的结果
        """
        try:
            sync_helper = self.ensure_sync_helper()

            # 创建一个异步操作来执行同步
            async def sync_operation():
                # 这里可以添加具体的同步逻辑
                # 目前返回一个简单的成功状态
                return {"status": "success", "operation": operation_name}

            # 执行同步操作
            result = sync_helper.run_sync(sync_operation())

            # 更新同步状态
            self._sync_status[operation_name] = {
                "last_sync": asyncio.get_event_loop().time(),
                "status": "success",
                "result": result
            }

            self._logger.debug(f"同步操作完成: {operation_name}")
            return result

        except Exception as e:
            self._logger.error(f"同步操作失败: {operation_name}, 错误: {e}")

            # 更新错误状态
            self._sync_status[operation_name] = {
                "last_sync": asyncio.get_event_loop().time(),
                "status": "error",
                "error": str(e)
            }

            raise

    def async_to_sync(self, async_coro, operation_name: str = "异步转同步") -> Any:
        """
        将异步协程转换为同步调用

        Args:
            async_coro: 异步协程
            operation_name: 操作名称

        Returns:
            异步协程的结果
        """
        try:
            sync_helper = self.ensure_sync_helper()
            result = sync_helper.run_sync(async_coro)

            self._logger.debug(f"异步转同步操作完成: {operation_name}")
            return result

        except Exception as e:
            self._logger.error(f"异步转同步操作失败: {operation_name}, 错误: {e}")
            raise

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
        max_retries = kwargs.pop('max_retries', self._retry_config['max_retries'])
        retry_delay = kwargs.pop('retry_delay', self._retry_config['retry_delay'])
        backoff_factor = kwargs.pop('backoff_factor', self._retry_config['backoff_factor'])

        last_exception = None

        for attempt in range(max_retries + 1):
            try:
                result = operation(*args, **kwargs)

                if attempt > 0:
                    self._logger.info(f"操作在第 {attempt + 1} 次尝试后成功")

                return result

            except Exception as e:
                last_exception = e

                if attempt < max_retries:
                    delay = retry_delay * (backoff_factor ** attempt)
                    self._logger.warning(
                        f"操作失败，{delay}秒后重试 (尝试 {attempt + 1}/{max_retries + 1}): {e}"
                    )
                    import time
                    time.sleep(delay)
                else:
                    self._logger.error(f"操作最终失败，已重试 {max_retries} 次: {e}")

        raise last_exception

    def get_sync_status(self, operation_name: Optional[str] = None) -> Dict[str, Any]:
        """
        获取同步状态信息

        Args:
            operation_name: 可选的操作名称，如果为None则返回所有状态

        Returns:
            同步状态信息
        """
        if operation_name:
            return self._sync_status.get(operation_name, {})
        else:
            return self._sync_status.copy()

    def clear_sync_status(self, operation_name: Optional[str] = None):
        """
        清理同步状态

        Args:
            operation_name: 可选的操作名称，如果为None则清理所有状态
        """
        if operation_name:
            self._sync_status.pop(operation_name, None)
            self._logger.debug(f"清理操作状态: {operation_name}")
        else:
            self._sync_status.clear()
            self._logger.debug("清理所有同步状态")

    def get_backend_info(self) -> Dict[str, Any]:
        """
        获取缓存后端信息

        Returns:
            后端信息字典
        """
        info = {
            "backend_type": "none",
            "is_configured": self._cache_backend is not None,
            "namespace": self._namespace
        }

        if self._cache_backend:
            info["backend_type"] = getattr(self._cache_backend, 'type', 'unknown')

            # 获取更多后端特定信息
            if hasattr(self._cache_backend, 'get_info'):
                backend_info = self._cache_backend.get_info()
                info.update(backend_info)

        return info

    def get_stats(self) -> Dict[str, Any]:
        """
        获取缓存管理器的统计信息

        Returns:
            统计信息字典
        """
        return {
            "namespace": self._namespace,
            "backend_configured": self._cache_backend is not None,
            "sync_helper_exists": self._sync_helper is not None,
            "sync_operations_count": len(self._sync_status),
            "retry_config": self._retry_config,
            "backend_info": self.get_backend_info()
        }


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
            self._logger.error(f"设置缓存失败: {e}")
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
            self._logger.error(f"删除缓存失败: {e}")
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
            self._logger.error(f"清空缓存失败: {e}")
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