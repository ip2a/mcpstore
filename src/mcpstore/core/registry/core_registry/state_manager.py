"""
State Manager - 状态管理模块

负责服务和工具状态的管理，包括：
1. 服务连接状态的设置和查询
2. 服务元数据的管理
3. 状态同步机制
4. 异步到同步操作的转换
"""

import asyncio
import logging
from typing import Dict, Any, Optional, List

from .base import StateManagerInterface
from .errors import raise_legacy_error

logger = logging.getLogger(__name__)


class StateManager(StateManagerInterface):
    """
    状态管理器实现

    职责：
    - 管理服务的连接状态
    - 处理服务的元数据
    - 提供状态同步机制
    - 处理异步到同步的转换
    """

    def __init__(self, cache_layer, naming_service, namespace: str = "default"):
        super().__init__(cache_layer, naming_service, namespace)

        # 状态同步管理器（懒加载）
        self._state_sync_manager = None

        # 同步助手（懒加载）
        self._sync_helper = None

        # 状态缓存
        self._state_cache = {}

        # 元数据缓存
        self._metadata_cache = {}

        # CacheLayerManager 实例（用于 pykv 操作）
        # 必须通过 set_cache_layer_manager() 方法设置
        self._cache_layer_manager = None

        self._logger.info(f"Initializing StateManager, namespace: {namespace}")

    def _legacy(self, method: str) -> None:
        raise_legacy_error(
            f"core_registry.StateManager.{method}",
            "Use mcpstore.core.cache.state_manager.StateManager via CacheLayerManager.",
        )

    def set_cache_layer_manager(self, cache_layer_manager) -> None:
        """
        设置 CacheLayerManager 实例

        StateManager 需要 CacheLayerManager 来执行 pykv 操作（如 get_state）。
        这个方法必须在使用 get_service_metadata_async 之前调用。

        Args:
            cache_layer_manager: CacheLayerManager 实例
        """
        self._cache_layer_manager = cache_layer_manager
        self._logger.debug("CacheLayerManager has been set")

    def initialize(self) -> None:
        """初始化状态管理器"""
        self._logger.info("StateManager initialization completed")

    def cleanup(self) -> None:
        """清理状态管理器资源"""
        try:
            # 清理缓存
            self._state_cache.clear()
            self._metadata_cache.clear()

            # 清理管理器
            if self._state_sync_manager:
                self._state_sync_manager = None

            self._sync_helper = None

            self._logger.info("StateManager cleanup completed")
        except Exception as e:
            self._logger.error(f"StateManager cleanup error: {e}")
            raise

    def set_service_state(self, agent_id: str, service_name: str, state: Optional['ServiceConnectionState']):
        """
        设置服务状态

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            state: 服务连接状态

        Note:
            使用内存缓存存储状态，不直接操作 pykv。
            pykv 状态由 cache/state_manager.py 管理。
        """
        self._legacy("set_service_state")

    def set_service_metadata(self, agent_id: str, service_name: str, metadata: Optional['ServiceStateMetadata']):
        """
        设置服务元数据（同步版本）

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            metadata: 服务状态元数据

        Note:
            使用 CacheLayerManager 的同步方法写入 pykv
        """
        self._legacy("set_service_metadata")

    def get_all_service_states(self, agent_id: str) -> Dict[str, 'ServiceConnectionState']:
        """
        获取指定agent_id的所有服务状态

        Args:
            agent_id: Agent ID

        Returns:
            服务名称到状态的映射
        """
        self._legacy("get_all_service_states")

    async def get_all_service_states_async(self, agent_id: str) -> Dict[str, 'ServiceConnectionState']:
        """
        异步获取指定agent_id的所有服务状态

        Args:
            agent_id: Agent ID

        Returns:
            服务名称到状态的映射
        """
        self._legacy("get_all_service_states_async")

    def get_connected_services(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        获取已连接的服务列表

        Args:
            agent_id: Agent ID

        Returns:
            已连接服务的信息列表
        """
        self._legacy("get_connected_services")

    def get_service_state(self, agent_id: str, service_name: str) -> Optional['ServiceConnectionState']:
        """
        获取指定服务的状态

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务状态或None
        """
        self._legacy("get_service_state")

    # [已删除] get_service_metadata 同步方法
    # 根据 "pykv 唯一真相数据源" 原则，所有元数据读取必须从 pykv 获取
    # 请使用 get_service_metadata_async 异步方法

    def sync_to_storage(self, operation, operation_name: str = "状态同步"):
        """
        同步执行异步操作

        Args:
            operation: 异步操作
            operation_name: 操作名称

        Returns:
            异步操作的结果
        """
        self._legacy("sync_to_storage")

    def _ensure_state_sync_manager(self):
        """
        确保状态同步管理器存在（懒加载）
        """
        self._legacy("_ensure_state_sync_manager")

    def _ensure_sync_helper(self):
        """
        确保同步助手存在（懒加载）
        """
        self._legacy("_ensure_sync_helper")

    async def _get_all_service_states_async_operation(self, agent_id: str) -> Dict[str, 'ServiceConnectionState']:
        """异步获取所有服务状态操作的包装"""
        self._legacy("_get_all_service_states_async_operation")

    def clear_agent_states(self, agent_id: str):
        """
        清除指定agent_id的所有状态缓存

        Args:
            agent_id: Agent ID
        """
        self._legacy("clear_agent_states")

    def get_services_by_state(self, agent_id: str, states: List['ServiceConnectionState']) -> List[str]:
        """
        根据状态获取服务列表

        Args:
            agent_id: Agent ID
            states: 状态列表

        Returns:
            符合条件的服务名称列表
        """
        self._legacy("get_services_by_state")

    def get_state_stats(self, agent_id: Optional[str] = None) -> Dict[str, Any]:
        """
        获取状态统计信息

        Args:
            agent_id: 可选的agent_id过滤

        Returns:
            状态统计信息
        """
        self._legacy("get_state_stats")

    async def get_service_metadata_async(self, agent_id: str, service_name: str) -> Optional['ServiceStateMetadata']:
        """
        异步获取服务元数据

        遵循 "pykv 唯一真相数据源" 原则，直接从 pykv 读取元数据。

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务状态元数据或None

        Raises:
            RuntimeError: 如果 CacheLayerManager 未设置
        """
        self._legacy("get_service_metadata_async")

    def get_stats(self) -> Dict[str, Any]:
        """
        获取状态管理器的统计信息

        Returns:
            统计信息字典
        """
        self._legacy("get_stats")

    def get_service_status(self, agent_id: str, service_name: str) -> Optional[str]:
        """
        获取服务状态（兼容性方法）

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务状态或None
        """
        self._legacy("get_service_status")


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
