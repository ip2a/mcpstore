"""
State Manager - 状态管理模块

负责服务和工具状态的管理，包括：
1. 服务连接状态的设置和查询
2. 服务元数据的管理
3. 状态同步机制
4. 异步到同步操作的转换
"""

import logging
import asyncio
from typing import Dict, Any, Optional, List
from datetime import datetime

from .base import StateManagerInterface

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

        self._logger.info(f"初始化StateManager，命名空间: {namespace}")

    def set_cache_layer_manager(self, cache_layer_manager) -> None:
        """
        设置 CacheLayerManager 实例

        StateManager 需要 CacheLayerManager 来执行 pykv 操作（如 get_state）。
        这个方法必须在使用 get_service_metadata_async 之前调用。

        Args:
            cache_layer_manager: CacheLayerManager 实例
        """
        self._cache_layer_manager = cache_layer_manager
        self._logger.debug("已设置 CacheLayerManager")

    def initialize(self) -> None:
        """初始化状态管理器"""
        self._logger.info("StateManager 初始化完成")

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

            self._logger.info("StateManager 清理完成")
        except Exception as e:
            self._logger.error(f"StateManager 清理时出错: {e}")
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
        # 更新本地缓存
        cache_key = f"{agent_id}:{service_name}"
        self._state_cache[cache_key] = state

        self._logger.debug(f"设置服务状态: {cache_key} = {state}")

    def set_service_metadata(self, agent_id: str, service_name: str, metadata: Optional['ServiceStateMetadata']):
        """
        设置服务元数据

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            metadata: 服务状态元数据

        Note:
            Functional Core: 纯同步操作，直接操作状态管理器
            不依赖已删除的 ServiceStateService
            保持API兼容性
        """
        try:
            # 生成全局名称
            global_name = self._naming.generate_service_global_name(service_name, agent_id)

            # 直接使用状态管理器设置元数据
            if metadata:
                self._cache_layer.set_entity_metadata(
                    entity_type="service",
                    global_name=global_name,
                    metadata=metadata
                )
            else:
                # 如果metadata为None，删除元数据
                self._cache_layer.delete_entity_metadata(
                    entity_type="service",
                    global_name=global_name
                )

            # 更新本地缓存
            cache_key = f"{agent_id}:{service_name}"
            if metadata:
                self._metadata_cache[cache_key] = metadata
            else:
                self._metadata_cache.pop(cache_key, None)

            self._logger.debug(f"设置服务元数据: {global_name}")

        except Exception as e:
            self._logger.error(f"设置服务元数据失败 {agent_id}:{service_name}: {e}")
            raise

    def get_all_service_states(self, agent_id: str) -> Dict[str, 'ServiceConnectionState']:
        """
        获取指定agent_id的所有服务状态

        Args:
            agent_id: Agent ID

        Returns:
            服务名称到状态的映射
        """
        try:
            # 检查缓存
            cached_states = {}
            cache_prefix = f"{agent_id}:"

            for cache_key, state in self._state_cache.items():
                if cache_key.startswith(cache_prefix):
                    service_name = cache_key.split(":", 1)[1]
                    cached_states[service_name] = state

            # 如果缓存为空或需要更新，从缓存层获取
            if not cached_states:
                return self.sync_to_storage(self._get_all_service_states_async_operation(agent_id))

            return cached_states

        except Exception as e:
            self._logger.error(f"获取所有服务状态失败 {agent_id}: {e}")
            return {}

    async def get_all_service_states_async(self, agent_id: str) -> Dict[str, 'ServiceConnectionState']:
        """
        异步获取指定agent_id的所有服务状态

        Args:
            agent_id: Agent ID

        Returns:
            服务名称到状态的映射
        """
        try:
            states = {}

            # 从缓存层获取所有服务状态
            service_states = await self._cache_layer.get_all_states_by_type("service")

            # 过滤指定agent_id的服务
            for global_name, state in service_states.items():
                # 从全局名称解析agent_id和service_name
                try:
                    service_name, parsed_agent_id = self._naming.parse_service_global_name(global_name)
                    if parsed_agent_id == agent_id:
                        states[service_name] = state
                except Exception:
                    # 解析失败，跳过
                    continue

            # 更新本地缓存
            for service_name, state in states.items():
                cache_key = f"{agent_id}:{service_name}"
                self._state_cache[cache_key] = state

            self._logger.debug(f"获取所有服务状态: agent={agent_id}, count={len(states)}")
            return states

        except Exception as e:
            self._logger.error(f"异步获取所有服务状态失败 {agent_id}: {e}")
            return {}

    def get_connected_services(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        获取已连接的服务列表

        Args:
            agent_id: Agent ID

        Returns:
            已连接服务的信息列表
        """
        try:
            # 获取所有服务状态
            all_states = self.get_all_service_states(agent_id)

            connected_services = []
            from ..models.service import ServiceConnectionState

            for service_name, state in all_states.items():
                if state == ServiceConnectionState.CONNECTED:
                    # 获取服务详细信息
                    global_name = self._naming.generate_service_global_name(service_name, agent_id)
                    service_info = self._cache_layer.get_entity_info("service", global_name)

                    connected_services.append({
                        "service_name": service_name,
                        "global_name": global_name,
                        "state": state,
                        "info": service_info
                    })

            self._logger.debug(f"获取已连接服务: agent={agent_id}, count={len(connected_services)}")
            return connected_services

        except Exception as e:
            self._logger.error(f"获取已连接服务失败 {agent_id}: {e}")
            return []

    def get_service_state(self, agent_id: str, service_name: str) -> Optional['ServiceConnectionState']:
        """
        获取指定服务的状态

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务状态或None
        """
        try:
            # 检查缓存
            cache_key = f"{agent_id}:{service_name}"
            if cache_key in self._state_cache:
                return self._state_cache[cache_key]

            # 缓存未命中，返回 None
            # 注意：状态数据应该通过 set_service_state 方法设置
            # 不从 _cache_layer 获取，因为 _cache_layer 可能是 kv_store 而不是 CacheLayerManager
            return None

        except Exception as e:
            self._logger.error(f"获取服务状态失败 {agent_id}:{service_name}: {e}")
            return None

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
        try:
            sync_helper = self._ensure_sync_helper()
            result = sync_helper.run_sync(operation)

            self._logger.debug(f"同步操作完成: {operation_name}")
            return result

        except Exception as e:
            self._logger.error(f"同步操作失败: {operation_name}, 错误: {e}")
            raise

    def _ensure_state_sync_manager(self):
        """
        确保状态同步管理器存在（懒加载）
        """
        if self._state_sync_manager is None:
            from ..sync.state_sync_manager import StateSyncManager
            self._state_sync_manager = StateSyncManager(self._cache_layer)
            self._logger.debug("创建了状态同步管理器")

        return self._state_sync_manager

    def _ensure_sync_helper(self):
        """
        确保同步助手存在（懒加载）
        """
        if self._sync_helper is None:
            self._sync_helper = AsyncSyncHelper()
            self._logger.debug("创建了异步同步助手")

        return self._sync_helper

    async def _get_all_service_states_async_operation(self, agent_id: str) -> Dict[str, 'ServiceConnectionState']:
        """异步获取所有服务状态操作的包装"""
        return await self.get_all_service_states_async(agent_id)

    def clear_agent_states(self, agent_id: str):
        """
        清除指定agent_id的所有状态缓存

        Args:
            agent_id: Agent ID
        """
        try:
            # 清理本地缓存
            keys_to_remove = []
            cache_prefix = f"{agent_id}:"

            for cache_key in self._state_cache:
                if cache_key.startswith(cache_prefix):
                    keys_to_remove.append(cache_key)

            for key in keys_to_remove:
                del self._state_cache[key]

            # 清理元数据缓存
            keys_to_remove = []
            for cache_key in self._metadata_cache:
                if cache_key.startswith(cache_prefix):
                    keys_to_remove.append(cache_key)

            for key in keys_to_remove:
                del self._metadata_cache[key]

            self._logger.info(f"清除agent状态缓存: {agent_id}")

        except Exception as e:
            self._logger.error(f"清除agent状态缓存失败 {agent_id}: {e}")

    def get_services_by_state(self, agent_id: str, states: List['ServiceConnectionState']) -> List[str]:
        """
        根据状态获取服务列表

        Args:
            agent_id: Agent ID
            states: 状态列表

        Returns:
            符合条件的服务名称列表
        """
        try:
            all_states = self.sync_to_storage(
                self.get_all_service_states_async(agent_id),
                "获取服务状态"
            )

            matching_services = []
            for service_name, state in all_states.items():
                if state in states:
                    matching_services.append(service_name)

            return matching_services

        except Exception as e:
            self._logger.error(f"根据状态获取服务列表失败 {agent_id}: {e}")
            return []

    def get_state_stats(self, agent_id: Optional[str] = None) -> Dict[str, Any]:
        """
        获取状态统计信息

        Args:
            agent_id: 可选的agent_id过滤

        Returns:
            状态统计信息
        """
        try:
            if agent_id:
                all_states = self.sync_to_storage(
                    self.get_all_service_states_async(agent_id),
                    "获取状态统计"
                )
            else:
                # 获取所有状态
                all_states = {}
                service_states = self.sync_to_storage(
                    self._cache_layer.get_all_states_by_type("service"),
                    "获取所有状态"
                )

                for global_name, state in service_states.items():
                    try:
                        service_name, parsed_agent_id = self._naming.parse_service_global_name(global_name)
                        all_states[f"{parsed_agent_id}:{service_name}"] = state
                    except Exception:
                        continue

            # 统计各状态的计数
            stats = {}
            for state in all_states.values():
                state_name = state.name if state else "None"
                stats[state_name] = stats.get(state_name, 0) + 1

            return {
                "total_services": len(all_states),
                "state_distribution": stats,
                "namespace": self._namespace,
                "agent_id": agent_id
            }

        except Exception as e:
            self._logger.error(f"获取状态统计失败: {e}")
            return {
                "total_services": 0,
                "state_distribution": {},
                "error": str(e)
            }

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
        # 检查 CacheLayerManager 是否已设置
        if self._cache_layer_manager is None:
            raise RuntimeError(
                "CacheLayerManager 未设置。"
                "请在使用 get_service_metadata_async 之前调用 set_cache_layer_manager() 方法。"
            )

        try:
            # 生成全局名称
            global_name = self._naming.generate_service_global_name(service_name, agent_id)

            # 从 pykv 读取元数据（使用 CacheLayerManager）
            metadata_dict = await self._cache_layer_manager.get_state("service_metadata", global_name)

            if metadata_dict is None:
                self._logger.debug(f"pykv 中未找到服务元数据: {global_name}")
                return None

            # 转换为 ServiceStateMetadata 对象（Pydantic v2 使用 model_validate）
            from mcpstore.core.models.service import ServiceStateMetadata
            metadata = ServiceStateMetadata.model_validate(metadata_dict)

            self._logger.debug(f"从 pykv 获取服务元数据成功: {global_name}")
            return metadata

        except Exception as e:
            self._logger.error(f"从 pykv 获取服务元数据失败 {agent_id}:{service_name}: {e}")
            raise

    def get_stats(self) -> Dict[str, Any]:
        """
        获取状态管理器的统计信息

        Returns:
            统计信息字典
        """
        return {
            "namespace": self._namespace,
            "state_cache_size": len(self._state_cache),
            "metadata_cache_size": len(self._metadata_cache),
            "has_state_sync_manager": self._state_sync_manager is not None,
            "has_sync_helper": self._sync_helper is not None
        }

    def get_service_status(self, agent_id: str, service_name: str) -> Optional[str]:
        """
        获取服务状态（兼容性方法）

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务状态或None
        """
        state = self.get_service_state(agent_id, service_name)
        return str(state) if state else None


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