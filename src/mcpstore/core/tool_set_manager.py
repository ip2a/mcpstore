"""
工具集管理器

本模块提供工具集管理的核心功能，负责维护 Agent 的工具集状态、
处理工具的增删改查操作，以及管理相关的缓存数据。

职责：
1. 维护 Agent 的当前可用工具集
2. 处理工具的增量添加和移除
3. 提供工具集的查询和重置
4. 管理工具集相关的索引和元数据
5. 处理服务名称映射关系
"""

import logging
from typing import Dict, Optional, Set, List, Any, TYPE_CHECKING
import time

from mcpstore.core.models.tool_set import ToolSetState

if TYPE_CHECKING:
    from key_value.aio.protocols import AsyncKeyValue
    from mcpstore.core.registry.core_registry import CoreRegistry

logger = logging.getLogger(__name__)


class ToolSetManager:
    """
    工具集管理器
    
    负责管理 Agent 的工具集状态，包括工具的添加、移除、重置等操作。
    所有数据通过 py-key-value 进行持久化，内存缓存仅用于性能优化。
    
    数据源唯一性原则：
    - py-key-value 是唯一的真实数据源（Single Source of Truth）
    - 内存缓存仅用于性能优化，不作为数据源
    - 所有读写操作都通过 py-key-value 进行
    
    Attributes:
        _kv_store: py-key-value 存储实例
        _registry: 服务注册表实例
        _memory_cache: 内存缓存字典（性能优化）
        _cache_ttl: 缓存过期时间（秒）
    """
    
    # 命名空间常量
    TOOL_SET_PREFIX = "tool_set"
    STATE_NS = f"{TOOL_SET_PREFIX}:state"              # 状态数据
    INDEX_NS = f"{TOOL_SET_PREFIX}:index"              # 索引数据
    METADATA_NS = f"{TOOL_SET_PREFIX}:metadata"        # 元数据
    MAPPING_NS = f"{TOOL_SET_PREFIX}:service_mapping"  # 服务映射
    REVERSE_NS = f"{TOOL_SET_PREFIX}:reverse_mapping"  # 反向映射
    
    # 缓存配置
    DEFAULT_CACHE_TTL = 3600  # 默认缓存过期时间：1小时
    
    def __init__(
        self,
        kv_store: 'AsyncKeyValue',
        registry: 'CoreRegistry',
        cache_ttl: int = DEFAULT_CACHE_TTL
    ):
        """
        初始化工具集管理器
        
        Args:
            kv_store: py-key-value 存储实例
            registry: 服务注册表实例
            cache_ttl: 缓存过期时间（秒），默认 3600 秒
        """
        self._kv_store = kv_store
        self._registry = registry
        self._cache_ttl = cache_ttl
        
        # 内存缓存：{cache_key: (state, timestamp)}
        self._memory_cache: Dict[str, tuple[ToolSetState, float]] = {}
        
        logger.info(
            f"ToolSetManager 初始化完成 "
            f"(cache_ttl={cache_ttl}s)"
        )
    
    def _get_cache_key(self, agent_id: str, service_name: str) -> str:
        """
        生成内存缓存键
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            
        Returns:
            缓存键字符串
        """
        return f"{agent_id}:{service_name}"
    
    def _is_cache_valid(self, timestamp: float) -> bool:
        """
        检查缓存是否有效
        
        Args:
            timestamp: 缓存时间戳
            
        Returns:
            True 如果缓存未过期，否则 False
        """
        return (time.time() - timestamp) < self._cache_ttl
    
    def _clear_memory_cache(self, agent_id: str, service_name: str) -> None:
        """
        清除内存缓存
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
        """
        cache_key = self._get_cache_key(agent_id, service_name)
        if cache_key in self._memory_cache:
            del self._memory_cache[cache_key]
            logger.debug(f"清除内存缓存: {cache_key}")

    # ==================== 缓存键生成方法 ====================
    
    def _get_state_key(self, agent_id: str, service_name: str) -> str:
        """
        生成工具集状态键
        
        格式: tool_set:state:{agent_id}:{service_name}
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            
        Returns:
            状态键字符串
            
        Examples:
            >>> manager._get_state_key("agent1", "weather")
            "tool_set:state:agent1:weather"
        """
        return f"{self.STATE_NS}:{agent_id}:{service_name}"
    
    def _get_index_key(self, agent_id: str) -> str:
        """
        生成 Agent 工具集索引键
        
        格式: tool_set:index:{agent_id}
        
        Args:
            agent_id: Agent ID
            
        Returns:
            索引键字符串
            
        Examples:
            >>> manager._get_index_key("agent1")
            "tool_set:index:agent1"
        """
        return f"{self.INDEX_NS}:{agent_id}"
    
    def _get_metadata_key(self, agent_id: str) -> str:
        """
        生成 Agent 工具集元数据键
        
        格式: tool_set:metadata:{agent_id}
        
        Args:
            agent_id: Agent ID
            
        Returns:
            元数据键字符串
            
        Examples:
            >>> manager._get_metadata_key("agent1")
            "tool_set:metadata:agent1"
        """
        return f"{self.METADATA_NS}:{agent_id}"
    
    def _get_mapping_key(self, agent_id: str, local_service_name: str) -> str:
        """
        生成服务映射键（正向映射：本地名称 -> 全局名称）
        
        格式: tool_set:service_mapping:{agent_id}:{local_service_name}
        
        Args:
            agent_id: Agent ID
            local_service_name: 本地服务名称
            
        Returns:
            映射键字符串
            
        Examples:
            >>> manager._get_mapping_key("agent1", "weather")
            "tool_set:service_mapping:agent1:weather"
        """
        return f"{self.MAPPING_NS}:{agent_id}:{local_service_name}"
    
    def _get_reverse_mapping_key(self, global_service_name: str) -> str:
        """
        生成反向映射键（反向映射：全局名称 -> Agent + 本地名称）
        
        格式: tool_set:reverse_mapping:{global_service_name}
        
        Args:
            global_service_name: 全局服务名称
            
        Returns:
            反向映射键字符串
            
        Examples:
            >>> manager._get_reverse_mapping_key("weather_byagent_agent1")
            "tool_set:reverse_mapping:weather_byagent_agent1"
        """
        return f"{self.REVERSE_NS}:{global_service_name}"

    # ==================== 工具集状态读取方法 ====================
    
    async def get_state_async(
        self,
        agent_id: str,
        service_name: str
    ) -> Optional[ToolSetState]:
        """
        获取工具集状态（异步）
        
        读取顺序：
        1. 先查内存缓存，命中则直接返回
        2. 未命中则从 py-key-value 读取
        3. 反序列化为 ToolSetState 对象
        4. 更新内存缓存
        5. 状态不存在时返回 None
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            
        Returns:
            ToolSetState 对象，如果不存在则返回 None
            
        Examples:
            >>> state = await manager.get_state_async("agent1", "weather")
            >>> if state:
            ...     print(f"可用工具: {state.available_tools}")
        """
        cache_key = self._get_cache_key(agent_id, service_name)
        
        # 1. 先查内存缓存
        if cache_key in self._memory_cache:
            state, timestamp = self._memory_cache[cache_key]
            if self._is_cache_valid(timestamp):
                logger.debug(f"内存缓存命中: {cache_key}")
                return state
            else:
                # 缓存过期，删除
                del self._memory_cache[cache_key]
                logger.debug(f"内存缓存过期: {cache_key}")
        
        # 2. 从 py-key-value 读取
        try:
            key = self._get_state_key(agent_id, service_name)
            data = await self._kv_store.get(key)
            
            if data is None:
                logger.debug(f"工具集状态不存在: {key}")
                return None
            
            # 3. 反序列化
            state = ToolSetState.from_dict(data)
            
            # 4. 更新内存缓存
            self._memory_cache[cache_key] = (state, time.time())
            logger.debug(f"从 py-key-value 加载工具集状态: {key}")
            
            return state
            
        except Exception as e:
            logger.error(
                f"读取工具集状态失败: agent_id={agent_id}, "
                f"service_name={service_name}, error={e}",
                exc_info=True
            )
            return None
    
    def get_state(
        self,
        agent_id: str,
        service_name: str
    ) -> Optional[ToolSetState]:
        """
        获取工具集状态（同步）
        
        这是 get_state_async 的同步版本，内部使用 asyncio 运行异步方法。
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            
        Returns:
            ToolSetState 对象，如果不存在则返回 None
            
        Examples:
            >>> state = manager.get_state("agent1", "weather")
            >>> if state:
            ...     print(f"可用工具: {state.available_tools}")
        """
        import asyncio
        
        try:
            # 尝试获取当前事件循环
            loop = asyncio.get_event_loop()
            if loop.is_running():
                # 如果循环正在运行，创建新任务
                import concurrent.futures
                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(
                        asyncio.run,
                        self.get_state_async(agent_id, service_name)
                    )
                    return future.result()
            else:
                # 如果循环未运行，直接运行
                return loop.run_until_complete(
                    self.get_state_async(agent_id, service_name)
                )
        except RuntimeError:
            # 没有事件循环，创建新的
            return asyncio.run(self.get_state_async(agent_id, service_name))

    # ==================== 工具集状态保存方法 ====================
    
    async def save_state_async(self, state: ToolSetState) -> None:
        """
        保存工具集状态（异步）
        
        保存流程：
        1. 验证 state 对象的必要字段
        2. 序列化为字典
        3. 写入 py-key-value
        4. 更新内存缓存
        5. 失败时记录警告但不中断
        
        Args:
            state: ToolSetState 对象
            
        Raises:
            ValueError: 如果 state 对象缺少必要字段
            
        Examples:
            >>> state = ToolSetState(
            ...     agent_id="agent1",
            ...     service_name="weather",
            ...     available_tools={"get_current", "get_forecast"}
            ... )
            >>> await manager.save_state_async(state)
        """
        # 1. 验证必要字段
        if not state.agent_id:
            raise ValueError("state.agent_id 不能为空")
        if not state.service_name:
            raise ValueError("state.service_name 不能为空")
        
        try:
            # 2. 序列化为字典
            data = state.to_dict()
            
            # 3. 写入 py-key-value
            key = self._get_state_key(state.agent_id, state.service_name)
            await self._kv_store.put(key, data)
            
            # 4. 更新内存缓存
            cache_key = self._get_cache_key(state.agent_id, state.service_name)
            self._memory_cache[cache_key] = (state, time.time())
            
            logger.debug(
                f"保存工具集状态成功: agent_id={state.agent_id}, "
                f"service_name={state.service_name}, "
                f"available_tools_count={len(state.available_tools)}"
            )
            
        except Exception as e:
            # 5. 失败时记录警告但不中断
            logger.warning(
                f"保存工具集状态失败: agent_id={state.agent_id}, "
                f"service_name={state.service_name}, error={e}",
                exc_info=True
            )
            # 不抛出异常，允许程序继续运行
    
    def save_state(self, state: ToolSetState) -> None:
        """
        保存工具集状态（同步）
        
        这是 save_state_async 的同步版本，内部使用 asyncio 运行异步方法。
        
        Args:
            state: ToolSetState 对象
            
        Raises:
            ValueError: 如果 state 对象缺少必要字段
            
        Examples:
            >>> state = ToolSetState(
            ...     agent_id="agent1",
            ...     service_name="weather",
            ...     available_tools={"get_current", "get_forecast"}
            ... )
            >>> manager.save_state(state)
        """
        import asyncio
        
        try:
            # 尝试获取当前事件循环
            loop = asyncio.get_event_loop()
            if loop.is_running():
                # 如果循环正在运行，创建新任务
                import concurrent.futures
                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(
                        asyncio.run,
                        self.save_state_async(state)
                    )
                    future.result()
            else:
                # 如果循环未运行，直接运行
                loop.run_until_complete(self.save_state_async(state))
        except RuntimeError:
            # 没有事件循环，创建新的
            asyncio.run(self.save_state_async(state))

    # ==================== 服务映射管理方法 ====================
    
    async def create_service_mapping_async(
        self,
        agent_id: str,
        local_name: str,
        global_name: str
    ) -> None:
        """
        创建服务映射（异步）
        
        创建本地服务名称到全局服务名称的双向映射关系。
        
        Args:
            agent_id: Agent ID
            local_name: 本地服务名称
            global_name: 全局服务名称
            
        Examples:
            >>> await manager.create_service_mapping_async(
            ...     "agent1", "weather", "weather_byagent_agent1"
            ... )
        """
        try:
            # 创建正向映射数据
            forward_mapping = {
                "agent_id": agent_id,
                "local_name": local_name,
                "global_name": global_name,
                "created_at": time.time(),
                "mapping_type": "agent_scoped",
                "is_active": True
            }
            
            # 创建反向映射数据
            reverse_mapping = {
                "global_name": global_name,
                "agent_id": agent_id,
                "local_name": local_name,
                "state_key": self._get_state_key(agent_id, local_name),
                "created_at": time.time()
            }
            
            # 保存正向映射
            forward_key = self._get_mapping_key(agent_id, local_name)
            await self._kv_store.put(forward_key, forward_mapping)
            
            # 保存反向映射
            reverse_key = self._get_reverse_mapping_key(global_name)
            await self._kv_store.put(reverse_key, reverse_mapping)
            
            logger.debug(
                f"创建服务映射成功: agent_id={agent_id}, "
                f"local_name={local_name}, global_name={global_name}"
            )
            
        except Exception as e:
            logger.error(
                f"创建服务映射失败: agent_id={agent_id}, "
                f"local_name={local_name}, global_name={global_name}, error={e}",
                exc_info=True
            )
            raise
    
    async def get_service_mapping_async(
        self,
        agent_id: str,
        local_name: str
    ) -> Optional[Dict[str, Any]]:
        """
        获取服务映射（异步）
        
        Args:
            agent_id: Agent ID
            local_name: 本地服务名称
            
        Returns:
            映射数据字典，如果不存在则返回 None
        """
        try:
            key = self._get_mapping_key(agent_id, local_name)
            mapping = await self._kv_store.get(key)
            
            if mapping is None:
                logger.debug(f"服务映射不存在: {key}")
                return None
            
            return mapping
            
        except Exception as e:
            logger.error(
                f"获取服务映射失败: agent_id={agent_id}, "
                f"local_name={local_name}, error={e}",
                exc_info=True
            )
            return None
    
    async def get_global_name_async(
        self,
        agent_id: str,
        local_name: str
    ) -> Optional[str]:
        """
        获取全局服务名称（异步）
        
        Args:
            agent_id: Agent ID
            local_name: 本地服务名称
            
        Returns:
            全局服务名称，如果不存在则返回 None
        """
        mapping = await self.get_service_mapping_async(agent_id, local_name)
        if mapping:
            return mapping.get("global_name")
        return None
    
    async def get_local_name_async(
        self,
        global_name: str
    ) -> Optional[tuple[str, str]]:
        """
        获取本地服务名称（异步）
        
        通过全局服务名称反向查询 Agent ID 和本地服务名称。
        
        Args:
            global_name: 全局服务名称
            
        Returns:
            (agent_id, local_name) 元组，如果不存在则返回 None
        """
        try:
            key = self._get_reverse_mapping_key(global_name)
            mapping = await self._kv_store.get(key)
            
            if mapping is None:
                logger.debug(f"反向映射不存在: {key}")
                return None
            
            return (mapping.get("agent_id"), mapping.get("local_name"))
            
        except Exception as e:
            logger.error(
                f"获取本地服务名称失败: global_name={global_name}, error={e}",
                exc_info=True
            )
            return None
    
    async def verify_service_mapping_async(
        self,
        agent_id: str,
        local_name: str
    ) -> bool:
        """
        验证服务映射是否存在（异步）
        
        Args:
            agent_id: Agent ID
            local_name: 本地服务名称
            
        Returns:
            True 如果映射存在，否则 False
        """
        mapping = await self.get_service_mapping_async(agent_id, local_name)
        return mapping is not None
    
    async def get_all_mappings_async(
        self,
        agent_id: str
    ) -> Dict[str, str]:
        """
        获取 Agent 的所有服务映射（异步）
        
        Args:
            agent_id: Agent ID
            
        Returns:
            映射字典 {local_name: global_name}
        """
        try:
            # 从索引中获取所有服务
            index_key = self._get_index_key(agent_id)
            index_data = await self._kv_store.get(index_key)
            
            if not index_data:
                return {}
            
            # 构建映射字典
            mappings = {}
            for entry in index_data:
                local_name = entry.get("service_name")
                global_name = entry.get("global_name")
                if local_name and global_name:
                    mappings[local_name] = global_name
            
            return mappings
            
        except Exception as e:
            logger.error(
                f"获取所有映射失败: agent_id={agent_id}, error={e}",
                exc_info=True
            )
            return {}
    
    async def delete_service_mapping_async(
        self,
        agent_id: str,
        local_name: str
    ) -> None:
        """
        删除服务映射（异步）
        
        Args:
            agent_id: Agent ID
            local_name: 本地服务名称
        """
        try:
            # 先获取 global_name
            global_name = await self.get_global_name_async(agent_id, local_name)
            
            if not global_name:
                logger.debug(f"映射不存在，无需删除: {agent_id}:{local_name}")
                return
            
            # 删除正向映射
            forward_key = self._get_mapping_key(agent_id, local_name)
            await self._kv_store.delete(forward_key)
            
            # 删除反向映射
            reverse_key = self._get_reverse_mapping_key(global_name)
            await self._kv_store.delete(reverse_key)
            
            logger.debug(
                f"删除服务映射成功: agent_id={agent_id}, "
                f"local_name={local_name}, global_name={global_name}"
            )
            
        except Exception as e:
            logger.error(
                f"删除服务映射失败: agent_id={agent_id}, "
                f"local_name={local_name}, error={e}",
                exc_info=True
            )
            raise

    # ==================== 索引管理方法 ====================
    
    async def _update_index_async(
        self,
        agent_id: str,
        service_name: str,
        tool_count: int,
        available_count: int
    ) -> None:
        """
        更新 Agent 工具集索引（异步）
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            tool_count: 总工具数
            available_count: 可用工具数
        """
        try:
            index_key = self._get_index_key(agent_id)
            
            # 读取现有索引
            index_data = await self._kv_store.get(index_key)
            if index_data is None:
                index_data = []
            
            # 查找或创建服务条目
            service_entry = None
            for entry in index_data:
                if entry.get("service_name") == service_name:
                    service_entry = entry
                    break
            
            # 获取全局名称
            global_name = await self.get_global_name_async(agent_id, service_name)
            
            if service_entry is None:
                # 创建新条目
                service_entry = {
                    "service_name": service_name,
                    "global_name": global_name or f"{service_name}_byagent_{agent_id}",
                    "state_key": self._get_state_key(agent_id, service_name),
                    "created_at": time.time(),
                    "tool_count": tool_count,
                    "available_count": available_count
                }
                index_data.append(service_entry)
            else:
                # 更新现有条目
                service_entry["tool_count"] = tool_count
                service_entry["available_count"] = available_count
                if global_name:
                    service_entry["global_name"] = global_name
            
            # 保存索引
            await self._kv_store.put(index_key, index_data)
            
            logger.debug(
                f"更新索引成功: agent_id={agent_id}, service_name={service_name}, "
                f"tool_count={tool_count}, available_count={available_count}"
            )
            
        except Exception as e:
            logger.warning(
                f"更新索引失败: agent_id={agent_id}, service_name={service_name}, error={e}",
                exc_info=True
            )
    
    async def _sync_index_async(self, agent_id: str) -> None:
        """
        同步 Agent 的完整索引（异步）
        
        重新计算并更新 Agent 的所有服务索引信息。
        
        Args:
            agent_id: Agent ID
        """
        try:
            index_key = self._get_index_key(agent_id)
            index_data = await self._kv_store.get(index_key)
            
            if index_data is None:
                logger.debug(f"索引不存在，无需同步: agent_id={agent_id}")
                return
            
            # 遍历所有服务，更新统计信息
            for entry in index_data:
                service_name = entry.get("service_name")
                if not service_name:
                    continue
                
                # 获取工具集状态
                state = await self.get_state_async(agent_id, service_name)
                if state:
                    # 从 registry 获取原始工具数
                    all_tools = await self._get_all_tools_async(agent_id, service_name)
                    entry["tool_count"] = len(all_tools)
                    entry["available_count"] = len(state.available_tools)
            
            # 保存更新后的索引
            await self._kv_store.put(index_key, index_data)
            
            logger.debug(f"同步索引成功: agent_id={agent_id}")
            
        except Exception as e:
            logger.warning(
                f"同步索引失败: agent_id={agent_id}, error={e}",
                exc_info=True
            )

    # ==================== 元数据管理方法 ====================
    
    async def _update_metadata_async(
        self,
        agent_id: str,
        operation_type: str,
        service_name: str
    ) -> None:
        """
        更新 Agent 工具集元数据（异步）
        
        Args:
            agent_id: Agent ID
            operation_type: 操作类型（"add", "remove", "reset", "initialize"）
            service_name: 服务名称
        """
        try:
            metadata_key = self._get_metadata_key(agent_id)
            
            # 读取现有元数据
            metadata = await self._kv_store.get(metadata_key)
            if metadata is None:
                # 创建新元数据
                metadata = {
                    "agent_id": agent_id,
                    "total_services": 0,
                    "total_original_tools": 0,
                    "total_available_tools": 0,
                    "overall_utilization": 0.0,
                    "created_at": time.time(),
                    "updated_at": time.time(),
                    "last_operation": {},
                    "statistics": {
                        "total_operations": 0,
                        "add_count": 0,
                        "remove_count": 0,
                        "reset_count": 0
                    }
                }
            
            # 更新最后操作记录
            metadata["last_operation"] = {
                "type": operation_type,
                "service": service_name,
                "timestamp": time.time()
            }
            metadata["updated_at"] = time.time()
            
            # 更新统计计数
            metadata["statistics"]["total_operations"] += 1
            if operation_type == "add":
                metadata["statistics"]["add_count"] += 1
            elif operation_type == "remove":
                metadata["statistics"]["remove_count"] += 1
            elif operation_type == "reset":
                metadata["statistics"]["reset_count"] += 1
            
            # 重新计算聚合统计
            await self._recalculate_metadata_stats(agent_id, metadata)
            
            # 保存元数据
            await self._kv_store.put(metadata_key, metadata)
            
            logger.debug(
                f"更新元数据成功: agent_id={agent_id}, "
                f"operation_type={operation_type}, service_name={service_name}"
            )
            
        except Exception as e:
            logger.warning(
                f"更新元数据失败: agent_id={agent_id}, "
                f"operation_type={operation_type}, error={e}",
                exc_info=True
            )
    
    async def _recalculate_metadata_stats(
        self,
        agent_id: str,
        metadata: Dict[str, Any]
    ) -> None:
        """
        重新计算元数据统计信息（异步）
        
        Args:
            agent_id: Agent ID
            metadata: 元数据字典（会被修改）
        """
        try:
            # 获取索引数据
            index_key = self._get_index_key(agent_id)
            index_data = await self._kv_store.get(index_key)
            
            if index_data is None:
                index_data = []
            
            # 计算聚合统计
            total_services = len(index_data)
            total_original_tools = sum(entry.get("tool_count", 0) for entry in index_data)
            total_available_tools = sum(entry.get("available_count", 0) for entry in index_data)
            
            # 计算利用率
            if total_original_tools > 0:
                overall_utilization = total_available_tools / total_original_tools
            else:
                overall_utilization = 0.0
            
            # 更新元数据
            metadata["total_services"] = total_services
            metadata["total_original_tools"] = total_original_tools
            metadata["total_available_tools"] = total_available_tools
            metadata["overall_utilization"] = round(overall_utilization, 2)
            
        except Exception as e:
            logger.warning(
                f"重新计算元数据统计失败: agent_id={agent_id}, error={e}",
                exc_info=True
            )
    
    async def _sync_metadata_async(self, agent_id: str) -> None:
        """
        同步 Agent 的元数据（异步）
        
        重新计算并更新 Agent 的所有元数据信息。
        
        Args:
            agent_id: Agent ID
        """
        try:
            metadata_key = self._get_metadata_key(agent_id)
            metadata = await self._kv_store.get(metadata_key)
            
            if metadata is None:
                logger.debug(f"元数据不存在，无需同步: agent_id={agent_id}")
                return
            
            # 重新计算统计信息
            await self._recalculate_metadata_stats(agent_id, metadata)
            
            # 保存更新后的元数据
            metadata["updated_at"] = time.time()
            await self._kv_store.put(metadata_key, metadata)
            
            logger.debug(f"同步元数据成功: agent_id={agent_id}")
            
        except Exception as e:
            logger.warning(
                f"同步元数据失败: agent_id={agent_id}, error={e}",
                exc_info=True
            )

    # ==================== 辅助方法 ====================
    
    async def _get_all_tools_async(
        self,
        agent_id: str,
        service_name: str
    ) -> Set[str]:
        """
        获取服务的所有原始工具（异步）
        
        从 Registry 获取服务的完整工具列表。
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            
        Returns:
            工具名称集合
        """
        try:
            # 从 registry 获取工具列表
            # 注意：这里需要根据实际的 registry API 调整
            tools = self._registry.list_tools(agent_id)
            
            # 筛选出属于该服务的工具
            service_tools = {
                t.name for t in tools
                if t.service_name == service_name
            }
            
            return service_tools
            
        except Exception as e:
            logger.error(
                f"获取服务工具列表失败: agent_id={agent_id}, "
                f"service_name={service_name}, error={e}",
                exc_info=True
            )
            return set()


    # ==================== 核心工具集操作方法 ====================
    
    async def add_tools_async(
        self,
        agent_id: str,
        service_name: str,
        tool_names: Any  # Union[List[str], Literal["_all_tools"]]
    ) -> None:
        """
        添加工具到可用集合（异步）
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            tool_names: 工具名称列表或 "_all_tools" 字符串
            
        Examples:
            >>> # 添加指定工具
            >>> await manager.add_tools_async("agent1", "weather", ["get_current", "get_forecast"])
            >>> 
            >>> # 添加所有工具
            >>> await manager.add_tools_async("agent1", "weather", "_all_tools")
        """
        try:
            # 处理 "_all_tools" 保留字符串
            if tool_names == "_all_tools":
                all_tools = await self._get_all_tools_async(agent_id, service_name)
                tools_to_add = all_tools
            else:
                tools_to_add = set(tool_names)
            
            # 获取或创建状态
            state = await self.get_state_async(agent_id, service_name)
            if state is None:
                # 创建新状态
                all_tools = await self._get_all_tools_async(agent_id, service_name)
                state = ToolSetState(
                    agent_id=agent_id,
                    service_name=service_name,
                    available_tools=set()
                )
            
            # 添加工具
            state.add_tools(tools_to_add)
            
            # 保存状态
            await self.save_state_async(state)
            
            # 更新索引
            all_tools = await self._get_all_tools_async(agent_id, service_name)
            await self._update_index_async(
                agent_id,
                service_name,
                len(all_tools),
                len(state.available_tools)
            )
            
            # 更新元数据
            await self._update_metadata_async(agent_id, "add", service_name)
            
            # 清除内存缓存
            self._clear_memory_cache(agent_id, service_name)
            
            logger.info(
                f"添加工具成功: agent_id={agent_id}, service_name={service_name}, "
                f"added_count={len(tools_to_add)}, "
                f"total_available={len(state.available_tools)}"
            )
            
        except Exception as e:
            logger.error(
                f"添加工具失败: agent_id={agent_id}, service_name={service_name}, error={e}",
                exc_info=True
            )
            raise
    
    def add_tools(
        self,
        agent_id: str,
        service_name: str,
        tool_names: Any
    ) -> None:
        """
        添加工具到可用集合（同步）
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            tool_names: 工具名称列表或 "_all_tools" 字符串
        """
        import asyncio
        
        try:
            loop = asyncio.get_event_loop()
            if loop.is_running():
                import concurrent.futures
                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(
                        asyncio.run,
                        self.add_tools_async(agent_id, service_name, tool_names)
                    )
                    future.result()
            else:
                loop.run_until_complete(
                    self.add_tools_async(agent_id, service_name, tool_names)
                )
        except RuntimeError:
            asyncio.run(self.add_tools_async(agent_id, service_name, tool_names))
    
    async def remove_tools_async(
        self,
        agent_id: str,
        service_name: str,
        tool_names: Any  # Union[List[str], Literal["_all_tools"]]
    ) -> None:
        """
        从可用集合移除工具（异步）
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            tool_names: 工具名称列表或 "_all_tools" 字符串
            
        Examples:
            >>> # 移除指定工具
            >>> await manager.remove_tools_async("agent1", "weather", ["get_history"])
            >>> 
            >>> # 移除所有工具
            >>> await manager.remove_tools_async("agent1", "weather", "_all_tools")
        """
        try:
            # 获取当前状态
            state = await self.get_state_async(agent_id, service_name)
            if state is None:
                logger.warning(
                    f"工具集状态不存在，无法移除工具: "
                    f"agent_id={agent_id}, service_name={service_name}"
                )
                return
            
            # 处理 "_all_tools" 保留字符串
            if tool_names == "_all_tools":
                tools_to_remove = state.available_tools.copy()
            else:
                tools_to_remove = set(tool_names)
            
            # 移除工具
            state.remove_tools(tools_to_remove)
            
            # 保存状态
            await self.save_state_async(state)
            
            # 更新索引
            all_tools = await self._get_all_tools_async(agent_id, service_name)
            await self._update_index_async(
                agent_id,
                service_name,
                len(all_tools),
                len(state.available_tools)
            )
            
            # 更新元数据
            await self._update_metadata_async(agent_id, "remove", service_name)
            
            # 清除内存缓存
            self._clear_memory_cache(agent_id, service_name)
            
            logger.info(
                f"移除工具成功: agent_id={agent_id}, service_name={service_name}, "
                f"removed_count={len(tools_to_remove)}, "
                f"remaining_available={len(state.available_tools)}"
            )
            
        except Exception as e:
            logger.error(
                f"移除工具失败: agent_id={agent_id}, service_name={service_name}, error={e}",
                exc_info=True
            )
            raise
    
    def remove_tools(
        self,
        agent_id: str,
        service_name: str,
        tool_names: Any
    ) -> None:
        """
        从可用集合移除工具（同步）
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            tool_names: 工具名称列表或 "_all_tools" 字符串
        """
        import asyncio
        
        try:
            loop = asyncio.get_event_loop()
            if loop.is_running():
                import concurrent.futures
                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(
                        asyncio.run,
                        self.remove_tools_async(agent_id, service_name, tool_names)
                    )
                    future.result()
            else:
                loop.run_until_complete(
                    self.remove_tools_async(agent_id, service_name, tool_names)
                )
        except RuntimeError:
            asyncio.run(self.remove_tools_async(agent_id, service_name, tool_names))
    
    async def reset_tools_async(
        self,
        agent_id: str,
        service_name: str
    ) -> None:
        """
        重置工具集为默认状态（异步）
        
        将工具集恢复为所有工具可用的状态。
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            
        Examples:
            >>> await manager.reset_tools_async("agent1", "weather")
        """
        try:
            # 获取所有原始工具
            all_tools = await self._get_all_tools_async(agent_id, service_name)
            
            # 获取或创建状态
            state = await self.get_state_async(agent_id, service_name)
            if state is None:
                state = ToolSetState(
                    agent_id=agent_id,
                    service_name=service_name,
                    available_tools=set()
                )
            
            # 重置为所有工具
            state.reset(all_tools)
            
            # 保存状态
            await self.save_state_async(state)
            
            # 更新索引
            await self._update_index_async(
                agent_id,
                service_name,
                len(all_tools),
                len(state.available_tools)
            )
            
            # 更新元数据
            await self._update_metadata_async(agent_id, "reset", service_name)
            
            # 清除内存缓存
            self._clear_memory_cache(agent_id, service_name)
            
            logger.info(
                f"重置工具集成功: agent_id={agent_id}, service_name={service_name}, "
                f"total_tools={len(all_tools)}"
            )
            
        except Exception as e:
            logger.error(
                f"重置工具集失败: agent_id={agent_id}, service_name={service_name}, error={e}",
                exc_info=True
            )
            raise
    
    def reset_tools(
        self,
        agent_id: str,
        service_name: str
    ) -> None:
        """
        重置工具集为默认状态（同步）
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
        """
        import asyncio
        
        try:
            loop = asyncio.get_event_loop()
            if loop.is_running():
                import concurrent.futures
                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(
                        asyncio.run,
                        self.reset_tools_async(agent_id, service_name)
                    )
                    future.result()
            else:
                loop.run_until_complete(
                    self.reset_tools_async(agent_id, service_name)
                )
        except RuntimeError:
            asyncio.run(self.reset_tools_async(agent_id, service_name))
    
    async def initialize_tool_set_async(
        self,
        agent_id: str,
        service_name: str,
        all_tools: List[str]
    ) -> None:
        """
        初始化工具集状态（异步）
        
        创建初始工具集状态，默认所有工具都可用。
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            all_tools: 所有工具名称列表
            
        Examples:
            >>> await manager.initialize_tool_set_async(
            ...     "agent1", "weather", ["get_current", "get_forecast", "get_history"]
            ... )
        """
        try:
            # 创建初始状态（默认全部可用）
            state = ToolSetState(
                agent_id=agent_id,
                service_name=service_name,
                available_tools=set(all_tools)
            )
            
            # 保存状态
            await self.save_state_async(state)
            
            # 创建服务映射（如果需要）
            global_name = f"{service_name}_byagent_{agent_id}"
            await self.create_service_mapping_async(agent_id, service_name, global_name)
            
            # 更新索引
            await self._update_index_async(
                agent_id,
                service_name,
                len(all_tools),
                len(all_tools)
            )
            
            # 更新元数据
            await self._update_metadata_async(agent_id, "initialize", service_name)
            
            logger.info(
                f"初始化工具集成功: agent_id={agent_id}, service_name={service_name}, "
                f"total_tools={len(all_tools)}"
            )
            
        except Exception as e:
            logger.error(
                f"初始化工具集失败: agent_id={agent_id}, service_name={service_name}, error={e}",
                exc_info=True
            )
            # 初始化失败不抛出异常，允许服务添加继续
    
    def initialize_tool_set(
        self,
        agent_id: str,
        service_name: str,
        all_tools: List[str]
    ) -> None:
        """
        初始化工具集状态（同步）
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            all_tools: 所有工具名称列表
        """
        import asyncio
        
        try:
            loop = asyncio.get_event_loop()
            if loop.is_running():
                import concurrent.futures
                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(
                        asyncio.run,
                        self.initialize_tool_set_async(agent_id, service_name, all_tools)
                    )
                    future.result()
            else:
                loop.run_until_complete(
                    self.initialize_tool_set_async(agent_id, service_name, all_tools)
                )
        except RuntimeError:
            asyncio.run(self.initialize_tool_set_async(agent_id, service_name, all_tools))

    # ==================== 服务清理方法 ====================
    
    async def cleanup_service_async(
        self,
        agent_id: str,
        service_name: str
    ) -> None:
        """
        清理单个服务的所有工具集数据（异步）
        
        清理内容：
        1. 删除工具集状态键
        2. 从索引中移除服务条目
        3. 更新元数据（减少服务计数）
        4. 删除服务映射键（正向和反向）
        5. 清除内存缓存
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            
        Examples:
            >>> await manager.cleanup_service_async("agent1", "weather")
        """
        try:
            logger.info(
                f"开始清理服务工具集数据: agent_id={agent_id}, service_name={service_name}"
            )
            
            # 1. 删除工具集状态键
            try:
                state_key = self._get_state_key(agent_id, service_name)
                await self._kv_store.delete(state_key)
                logger.debug(f"删除工具集状态键: {state_key}")
            except Exception as e:
                logger.warning(f"删除工具集状态键失败: {e}")
            
            # 2. 从索引中移除服务条目
            try:
                index_key = self._get_index_key(agent_id)
                index_data = await self._kv_store.get(index_key)
                
                if index_data:
                    # 过滤掉要删除的服务
                    index_data = [
                        entry for entry in index_data
                        if entry.get("service_name") != service_name
                    ]
                    await self._kv_store.put(index_key, index_data)
                    logger.debug(f"从索引中移除服务: {service_name}")
            except Exception as e:
                logger.warning(f"从索引中移除服务失败: {e}")
            
            # 3. 更新元数据（减少服务计数）
            try:
                metadata_key = self._get_metadata_key(agent_id)
                metadata = await self._kv_store.get(metadata_key)
                
                if metadata:
                    # 减少服务计数
                    metadata["total_services"] = max(0, metadata.get("total_services", 1) - 1)
                    metadata["updated_at"] = time.time()
                    
                    # 重新计算统计信息
                    await self._recalculate_metadata_stats(agent_id, metadata)
                    
                    await self._kv_store.put(metadata_key, metadata)
                    logger.debug(f"更新元数据: 服务计数减少")
            except Exception as e:
                logger.warning(f"更新元数据失败: {e}")
            
            # 4. 删除服务映射键（正向和反向）
            try:
                # 获取全局名称
                mapping = await self.get_service_mapping_async(agent_id, service_name)
                if mapping:
                    global_name = mapping.get("global_name")
                    
                    # 删除正向映射
                    forward_key = self._get_mapping_key(agent_id, service_name)
                    await self._kv_store.delete(forward_key)
                    logger.debug(f"删除正向映射: {forward_key}")
                    
                    # 删除反向映射
                    if global_name:
                        reverse_key = self._get_reverse_mapping_key(global_name)
                        await self._kv_store.delete(reverse_key)
                        logger.debug(f"删除反向映射: {reverse_key}")
            except Exception as e:
                logger.warning(f"删除服务映射失败: {e}")
            
            # 5. 清除内存缓存
            try:
                self._clear_memory_cache(agent_id, service_name)
                logger.debug(f"清除内存缓存: {agent_id}:{service_name}")
            except Exception as e:
                logger.warning(f"清除内存缓存失败: {e}")
            
            logger.info(
                f"清理服务工具集数据完成: agent_id={agent_id}, service_name={service_name}"
            )
            
        except Exception as e:
            # 清理失败记录警告但不中断
            logger.warning(
                f"清理服务工具集数据失败: agent_id={agent_id}, "
                f"service_name={service_name}, error={e}",
                exc_info=True
            )
    
    def cleanup_service(
        self,
        agent_id: str,
        service_name: str
    ) -> None:
        """
        清理单个服务的所有工具集数据（同步）
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
        """
        import asyncio
        
        try:
            loop = asyncio.get_event_loop()
            if loop.is_running():
                import concurrent.futures
                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(
                        asyncio.run,
                        self.cleanup_service_async(agent_id, service_name)
                    )
                    future.result()
            else:
                loop.run_until_complete(
                    self.cleanup_service_async(agent_id, service_name)
                )
        except RuntimeError:
            asyncio.run(self.cleanup_service_async(agent_id, service_name))
