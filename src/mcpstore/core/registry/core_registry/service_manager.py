"""
Service Manager - 服务管理模块

负责服务的完整生命周期管理，包括：
1. 服务注册和注销
2. 服务状态管理
3. 工具管理和服务关联
4. 服务配置管理
5. 长生命周期连接管理
"""

import logging
from datetime import datetime
from typing import Dict, Any, Optional, List, Tuple

from .base import ServiceManagerInterface

logger = logging.getLogger(__name__)


class ServiceManager(ServiceManagerInterface):
    """
    服务管理器实现

    职责：
    - 管理服务的注册、注销和更新
    - 处理服务状态和配置
    - 管理服务与工具的关联
    - 处理长生命周期连接
    """

    def __init__(self, cache_layer, naming_service, namespace: str = "default"):
        super().__init__(cache_layer, naming_service, namespace)

        # 管理器引用（将在后续注入）
        self._service_entity_manager = None
        self._relation_manager = None
        self._tool_manager = None
        self._state_manager = None
        self._session_manager = None
        self._cache_manager = None
        self._mapping_manager = None

        # 长生命周期连接标记
        self.long_lived_connections: set = set()

        # 服务缓存
        self._service_cache = {}

        self._logger.info(f"[SERVICE_MANAGER] [INIT] Initializing ServiceManager, namespace: {namespace}")

    def initialize(self) -> None:
        """初始化服务管理器"""
        self._logger.info("[SERVICE_MANAGER] [INIT] ServiceManager initialization completed")

    def cleanup(self) -> None:
        """清理服务管理器资源"""
        try:
            # 清理缓存
            self._service_cache.clear()
            self.long_lived_connections.clear()

            # 清理管理器引用
            self._service_entity_manager = None
            self._relation_manager = None
            self._tool_manager = None
            self._state_manager = None
            self._session_manager = None
            self._cache_manager = None

            self._logger.info("[SERVICE_MANAGER] [CLEAN] ServiceManager cleanup completed")
        except Exception as e:
            self._logger.error(f"[SERVICE_MANAGER] [ERROR] ServiceManager cleanup error: {e}")
            raise

    def set_managers(self, service_entity_manager=None, relation_manager=None,
                     tool_manager=None, state_manager=None, session_manager=None,
                     cache_manager=None, mapping_manager=None, tool_entity_manager=None):
        """
        设置依赖的管理器

        Args:
            service_entity_manager: 服务实体管理器
            relation_manager: 关系管理器
            tool_manager: 工具管理器
            state_manager: 状态管理器
            session_manager: 会话管理器
            cache_manager: 缓存管理器
            mapping_manager: 映射管理器
            tool_entity_manager: 工具实体管理器
        """
        self._service_entity_manager = service_entity_manager
        self._relation_manager = relation_manager
        self._tool_manager = tool_manager
        self._state_manager = state_manager
        self._session_manager = session_manager
        self._cache_manager = cache_manager
        self._mapping_manager = mapping_manager
        self._tool_entity_manager = tool_entity_manager
        self._logger.info("[SERVICE_MANAGER] [SET] Dependent managers have been set")

    def add_service(self, agent_id: str, name: str, session: Any = None,
                   tools: List[Tuple[str, Dict[str, Any]]] = None,
                   service_config: Dict[str, Any] = None,
                   auto_connect: bool = True) -> bool:
        """
        添加服务

        Args:
            agent_id: Agent ID
            name: 服务名称
            session: 服务会话对象
            tools: 工具列表 [(tool_name, tool_def)]
            service_config: 服务配置
            auto_connect: 是否自动连接

        Returns:
            是否成功添加
        """
        try:
            tools = tools or []
            service_config = service_config or {}

            # 生成全局名称
            service_global_name = self._naming.generate_service_global_name(name, agent_id)

            # 确定服务状态
            if session is not None and len(tools) > 0:
                from mcpstore.core.models.service import ServiceConnectionState
                state = ServiceConnectionState.HEALTHY
            elif session is not None:
                from mcpstore.core.models.service import ServiceConnectionState
                state = ServiceConnectionState.DEGRADED
            else:
                from mcpstore.core.models.service import ServiceConnectionState
                state = ServiceConnectionState.DISCONNECTED

            # 检查服务是否已存在
            service_exists = False
            if self._service_entity_manager:
                existing_service = self._sync_operation(
                    self._service_entity_manager.get_service(service_global_name),
                    f"check_service_exists:{service_global_name}"
                )
                if existing_service:
                    logger.debug(f"[SERVICE_MANAGER] [EXISTS] Service already exists: {service_global_name}, will update tools")
                    service_exists = True

            # 创建服务实体（仅当服务不存在时）
            if not service_exists and self._service_entity_manager:
                self._sync_operation(
                    self._service_entity_manager.create_service(
                        agent_id=agent_id,
                        original_name=name,
                        config=service_config
                    ),
                    f"create_service:{service_global_name}"
                )

            # 创建Agent-Service关系（仅当服务不存在时）
            if not service_exists and self._relation_manager:
                client_id = f"client_{agent_id}_{name}"
                self._sync_operation(
                    self._relation_manager.add_agent_service(
                        agent_id=agent_id,
                        service_original_name=name,
                        service_global_name=service_global_name,
                        client_id=client_id
                    ),
                    f"add_agent_service:{agent_id}:{service_global_name}"
                )

            # 设置服务状态
            if self._state_manager:
                self._state_manager.set_service_state(agent_id, name, state)

            # 设置服务会话
            if self._session_manager and session:
                self._session_manager.set_session(agent_id, name, session)

                # 设置工具会话映射
                for tool_name, tool_def in tools:
                    self._session_manager.add_tool_session_mapping(agent_id, tool_name, session)

            # 添加工具
            self._logger.info(f"[ADD_SERVICE] [CHECK] Checking tool addition conditions: _tool_manager={self._tool_manager is not None}, tools={tools is not None}, tools_count={len(tools) if tools else 0}")
            if self._tool_manager and tools:
                self._add_tools_to_service(agent_id, name, tools)
            else:
                self._logger.warning(f"[ADD_SERVICE] [SKIP] Skipping tool addition: _tool_manager={self._tool_manager}, tools={tools}")

            # 更新缓存
            cache_key = f"{agent_id}:{name}"
            self._service_cache[cache_key] = {
                "name": name,
                "global_name": service_global_name,
                "state": state,
                "config": service_config,
                "added_time": datetime.now()
            }

            self._logger.info(f"[SERVICE_MANAGER] [SUCCESS] Service added successfully: {service_global_name}")
            return True

        except Exception as e:
            self._logger.error(f"[SERVICE_MANAGER] [ERROR] Failed to add service {agent_id}:{name}: {e}")
            return False

    async def add_service_async(self, agent_id: str, name: str, session: Any = None,
                              tools: List[Tuple[str, Dict[str, Any]]] = None,
                              service_config: Dict[str, Any] = None,
                              auto_connect: bool = True) -> bool:
        """
        异步添加服务

        遵循 "Functional Core, Imperative Shell" 架构原则：
        - 异步外壳直接使用 await 调用异步操作
        - 不通过 _sync_operation 转换

        Args:
            agent_id: Agent ID
            name: 服务名称
            session: 服务会话对象
            tools: 工具列表
            service_config: 服务配置
            auto_connect: 是否自动连接

        Returns:
            是否成功添加
        """
        try:
            tools = tools or []
            service_config = service_config or {}

            # 生成全局名称
            service_global_name = self._naming.generate_service_global_name(name, agent_id)

            # 确定服务状态
            if session is not None and len(tools) > 0:
                from mcpstore.core.models.service import ServiceConnectionState
                state = ServiceConnectionState.HEALTHY
            elif session is not None:
                from mcpstore.core.models.service import ServiceConnectionState
                state = ServiceConnectionState.DEGRADED
            else:
                from mcpstore.core.models.service import ServiceConnectionState
                state = ServiceConnectionState.DISCONNECTED

            # 检查服务是否已存在（异步）
            service_exists = False
            if self._service_entity_manager:
                existing_service = await self._service_entity_manager.get_service(service_global_name)
                if existing_service:
                    logger.debug(f"[SERVICE_MANAGER] [EXISTS] Service already exists: {service_global_name}, will update tools")
                    service_exists = True

            # 创建服务实体（仅当服务不存在时）
            if not service_exists and self._service_entity_manager:
                await self._service_entity_manager.create_service(
                    agent_id=agent_id,
                    original_name=name,
                    config=service_config
                )

            # 创建Agent-Service关系（仅当服务不存在时）
            if not service_exists and self._relation_manager:
                client_id = f"client_{agent_id}_{name}"
                await self._relation_manager.add_agent_service(
                    agent_id=agent_id,
                    service_original_name=name,
                    service_global_name=service_global_name,
                    client_id=client_id
                )

            # 设置服务状态（同步操作，使用内存缓存）
            if self._state_manager:
                self._state_manager.set_service_state(agent_id, name, state)

            # 设置服务会话（同步操作，使用内存缓存）
            if self._session_manager and session:
                self._session_manager.set_session(agent_id, name, session)

                # 设置工具会话映射
                for tool_name, tool_def in tools:
                    self._session_manager.add_tool_session_mapping(agent_id, tool_name, session)

            # 添加工具（异步）
            self._logger.info(f"[ADD_SERVICE_ASYNC] [CHECK] Checking tool addition conditions: _tool_manager={self._tool_manager is not None}, tools={tools is not None}, tools_count={len(tools) if tools else 0}")
            if self._tool_manager and tools:
                await self._add_tools_to_service_async(agent_id, name, tools)
            else:
                self._logger.warning(f"[ADD_SERVICE_ASYNC] Skipping tool addition: _tool_manager={self._tool_manager}, tools={tools}")

            # 更新缓存（同步操作，使用内存缓存）
            cache_key = f"{agent_id}:{name}"
            self._service_cache[cache_key] = {
                "name": name,
                "global_name": service_global_name,
                "state": state,
                "config": service_config,
                "added_time": datetime.now()
            }

            self._logger.info(f"[SERVICE_MANAGER] [SUCCESS] Async service addition successful: {service_global_name}")
            return True

        except Exception as e:
            self._logger.error(f"[SERVICE_MANAGER] [ERROR] Async service addition failed {agent_id}:{name}: {e}")
            return False

    def remove_service(self, agent_id: str, name: str) -> Optional[Any]:
        """
        移除服务

        Args:
            agent_id: Agent ID
            name: 服务名称

        Returns:
            被移除的会话对象
        """
        try:
            service_global_name = self._naming.generate_service_global_name(name, agent_id)
            removed_session = None

            # 获取会话对象
            if self._session_manager:
                removed_session = self._session_manager.get_session(agent_id, name)

            # 移除服务实体
            if self._service_entity_manager:
                self._sync_operation(
                    self._service_entity_manager.delete_service(service_global_name),
                    f"delete_service:{service_global_name}"
                )

            # 移除关系
            if self._relation_manager:
                self._sync_operation(
                    self._relation_manager.remove_agent_service(agent_id, service_global_name),
                    f"remove_agent_service:{agent_id}:{service_global_name}"
                )

            # 清理会话
            if self._session_manager:
                self._session_manager.clear_session(agent_id, name)

            # 清理状态
            if self._state_manager:
                self._state_manager.set_service_state(agent_id, name, None)

            # 清理缓存
            cache_key = f"{agent_id}:{name}"
            self._service_cache.pop(cache_key, None)

            # 移除长生命周期标记
            connection_id = f"{agent_id}:{name}"
            self.long_lived_connections.discard(connection_id)

            self._logger.info(f"[SERVICE_MANAGER] [SUCCESS] Service removal successful: {service_global_name}")
            return removed_session

        except Exception as e:
            self._logger.error(f"[SERVICE_MANAGER] [ERROR] Service removal failed {agent_id}:{name}: {e}")
            return None

    async def remove_service_async(self, agent_id: str, name: str) -> Optional[Any]:
        """
        异步移除服务

        Args:
            agent_id: Agent ID
            name: 服务名称

        Returns:
            被移除的会话对象
        """
        # 简化实现：同步调用
        return self.remove_service(agent_id, name)

    def replace_service_tools(self, agent_id: str, service_name: str, session: Any,
                            remote_tools: List[Any]) -> Dict[str, Any]:
        """
        替换服务工具

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            session: 服务会话
            remote_tools: 远程工具列表

        Returns:
            替换结果统计
        """
        try:
            service_global_name = self._naming.generate_service_global_name(service_name, agent_id)

            # 清理现有工具映射
            if self._session_manager:
                self._session_manager.clear_session(agent_id, service_name)

            # 设置新会话
            if self._session_manager and session:
                self._session_manager.set_session(agent_id, service_name, session)

            # 处理远程工具
            processed_tools = []
            for remote_tool in remote_tools:
                if hasattr(remote_tool, 'name') and hasattr(remote_tool, 'schema'):
                    tool_def = {
                        "name": remote_tool.name,
                        "description": getattr(remote_tool, 'description', ''),
                        "inputSchema": remote_tool.schema
                    }
                    processed_tools.append((remote_tool.name, tool_def))

                    # 更新工具会话映射
                    if self._session_manager:
                        self._session_manager.add_tool_session_mapping(
                            agent_id, remote_tool.name, session
                        )

            # 添加工具到服务
            if self._tool_manager and processed_tools:
                self._add_tools_to_service(agent_id, service_name, processed_tools)

            # 更新服务状态为健康
            if self._state_manager:
                from mcpstore.core.models.service import ServiceConnectionState
                self._state_manager.set_service_state(
                    agent_id, service_name, ServiceConnectionState.HEALTHY
                )

            result = {
                "service": service_global_name,
                "tools_processed": len(processed_tools),
                "status": "success"
            }

            self._logger.info(f"[SERVICE_MANAGER] [SUCCESS] Service tools replacement successful: {service_global_name}, tools_count: {len(processed_tools)}")
            return result

        except Exception as e:
            self._logger.error(f"[SERVICE_MANAGER] [ERROR] Service tools replacement failed {agent_id}:{service_name}: {e}")
            return {
                "service": service_name,
                "tools_processed": 0,
                "status": "failed",
                "error": str(e)
            }

    async def replace_service_tools_async(self, agent_id: str, service_name: str, session: Any,
                                         remote_tools: List[Any]) -> Dict[str, Any]:
        """
        异步替换服务工具

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            session: 服务会话
            remote_tools: 远程工具列表

        Returns:
            替换结果统计
        """
        # 简化实现：同步调用
        return self.replace_service_tools(agent_id, service_name, session, remote_tools)

    def add_failed_service(self, agent_id: str, name: str, service_config: Dict[str, Any],
                         error_info: Optional[Dict[str, Any]] = None) -> bool:
        """
        添加失败的服务

        Args:
            agent_id: Agent ID
            name: 服务名称
            service_config: 服务配置
            error_info: 错误信息

        Returns:
            是否成功添加
        """
        try:
            # 添加服务，但没有会话和工具
            return self.add_service(
                agent_id=agent_id,
                name=name,
                session=None,
                tools=[],
                service_config=service_config,
                auto_connect=False
            )

        except Exception as e:
            self._logger.error(f"Failed to add service {agent_id}:{name}: {e}")
            return False

    def get_services_for_agent(self, agent_id: str) -> List[str]:
        """
        获取指定agent的所有服务

        Args:
            agent_id: Agent ID

        Returns:
            服务名称列表
        """
        try:
            # 从关系管理器获取服务
            if self._relation_manager:
                services = self._sync_operation(
                    self._relation_manager.get_agent_services(agent_id),
                    f"get_agent_services:{agent_id}"
                )
                return [service.get("service_original_name", "") for service in services]

            # 从缓存获取
            service_names = []
            cache_prefix = f"{agent_id}:"
            for cache_key in self._service_cache:
                if cache_key.startswith(cache_prefix):
                    service_name = cache_key.split(":", 1)[1]
                    service_names.append(service_name)

            return service_names

        except Exception as e:
            self._logger.error(f"Failed to get agent service list {agent_id}: {e}")
            return []

    async def get_services_for_agent_async(self, agent_id: str) -> List[str]:
        """
        异步获取指定agent的所有服务

        遵循 "Functional Core, Imperative Shell" 架构原则：
        - 异步外壳直接使用 await 调用异步操作
        - 不通过 _sync_operation 转换

        Args:
            agent_id: Agent ID

        Returns:
            服务名称列表
        """
        try:
            # 从关系管理器获取服务（异步）
            if self._relation_manager:
                services = await self._relation_manager.get_agent_services(agent_id)
                return [service.get("service_original_name", "") for service in services]

            # 从缓存获取（同步操作，使用内存缓存）
            service_names = []
            cache_prefix = f"{agent_id}:"
            for cache_key in self._service_cache:
                if cache_key.startswith(cache_prefix):
                    service_name = cache_key.split(":", 1)[1]
                    service_names.append(service_name)

            return service_names

        except Exception as e:
            self._logger.error(f"Failed to get agent service list asynchronously {agent_id}: {e}")
            raise

    def get_service_details(self, agent_id: str, name: str) -> Dict[str, Any]:
        """
        获取服务详细信息

        Args:
            agent_id: Agent ID
            name: 服务名称

        Returns:
            服务详细信息
        """
        try:
            service_global_name = self._naming.generate_service_global_name(name, agent_id)

            # 获取基础信息
            cache_key = f"{agent_id}:{name}"
            cached_info = self._service_cache.get(cache_key, {})

            # 获取服务实体信息
            service_info = {}
            if self._service_entity_manager:
                service_entity = self._sync_operation(
                    self._service_entity_manager.get_service(service_global_name),
                    f"get_service:{service_global_name}"
                )
                if service_entity:
                    service_info = {
                        "global_name": service_global_name,
                        "original_name": service_entity.service_original_name,
                        "config": service_entity.config,
                        "added_time": service_entity.added_time
                    }

            # 获取状态信息
            state = None
            if self._state_manager:
                state = self._state_manager.get_service_state(agent_id, name)

            # 获取工具信息
            tools = []
            if self._tool_manager:
                tools = self._tool_manager.get_tools_for_service(agent_id, name)

            # 获取会话信息
            has_session = False
            if self._session_manager:
                has_session = self._session_manager.has_session(agent_id, name)

            # 组合详细信息
            details = {
                **cached_info,
                **service_info,
                "state": state,
                "tools": tools,
                "has_session": has_session,
                "is_long_lived": self.is_long_lived_service(agent_id, name)
            }

            return details

        except Exception as e:
            self._logger.error(f"Failed to get service details {agent_id}:{name}: {e}")
            return {}

    def get_service_info(self, agent_id: str, service_name: str) -> Optional['ServiceInfo']:
        """
        获取服务信息

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            ServiceInfo对象或None
        """
        try:
            details = self.get_service_details(agent_id, service_name)
            if not details:
                return None

            # 创建ServiceInfo对象
            return {
                "name": service_name,
                "global_name": details.get("global_name"),
                "state": details.get("state"),
                "config": details.get("config"),
                "tools_count": len(details.get("tools", [])),
                "has_session": details.get("has_session", False),
                "is_long_lived": details.get("is_long_lived", False)
            }

        except Exception as e:
            self._logger.error(f"Failed to get service info {agent_id}:{service_name}: {e}")
            return None

    def get_service_config(self, agent_id: str, name: str) -> Optional[Dict[str, Any]]:
        """
        获取服务配置

        Args:
            agent_id: Agent ID
            name: 服务名称

        Returns:
            服务配置或None
        """
        try:
            # 从缓存获取
            cache_key = f"{agent_id}:{name}"
            cached_info = self._service_cache.get(cache_key)
            if cached_info and "config" in cached_info:
                return cached_info["config"]

            # 从实体管理器获取
            if self._service_entity_manager:
                service_global_name = self._naming.generate_service_global_name(name, agent_id)
                service_entity = self._sync_operation(
                    self._service_entity_manager.get_service(service_global_name),
                    f"get_service_config:{service_global_name}"
                )
                if service_entity:
                    return service_entity.config

            return None

        except Exception as e:
            self._logger.error(f"Failed to get service config {agent_id}:{name}: {e}")
            return None

    def mark_as_long_lived(self, agent_id: str, service_name: str):
        """
        标记为长生命周期连接

        Args:
            agent_id: Agent ID
            service_name: 服务名称
        """
        connection_id = f"{agent_id}:{service_name}"
        self.long_lived_connections.add(connection_id)
        self._logger.debug(f"Marking long-lived connection: {connection_id}")

    def is_long_lived_service(self, agent_id: str, service_name: str) -> bool:
        """
        检查是否为长生命周期服务

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            是否为长生命周期服务
        """
        connection_id = f"{agent_id}:{service_name}"
        return connection_id in self.long_lived_connections

    def get_long_lived_services(self, agent_id: str) -> List[str]:
        """
        获取长生命周期服务列表

        Args:
            agent_id: Agent ID

        Returns:
            长生命周期服务名称列表
        """
        long_lived = []
        prefix = f"{agent_id}:"

        for connection_id in self.long_lived_connections:
            if connection_id.startswith(prefix):
                service_name = connection_id.split(":", 1)[1]
                long_lived.append(service_name)

        return long_lived

    def remove_service_lifecycle_data(self, agent_id: str, service_name: str):
        """
        移除服务生命周期数据

        Args:
            agent_id: Agent ID
            service_name: 服务名称
        """
        try:
            # 移除长生命周期标记
            connection_id = f"{agent_id}:{service_name}"
            self.long_lived_connections.discard(connection_id)

            # 清理缓存
            cache_key = f"{agent_id}:{service_name}"
            self._service_cache.pop(cache_key, None)

            self._logger.debug(f"Removing service lifecycle data: {connection_id}")

        except Exception as e:
            self._logger.error(f"Failed to remove service lifecycle data {agent_id}:{service_name}: {e}")

    def clear(self, agent_id: str):
        """
        清除指定agent的所有服务

        Args:
            agent_id: Agent ID
        """
        try:
            # 获取所有服务
            services = self.get_services_for_agent(agent_id)

            # 移除所有服务
            for service_name in services:
                self.remove_service(agent_id, service_name)

            # 清理长生命周期连接
            prefix = f"{agent_id}:"
            to_remove = []
            for connection_id in self.long_lived_connections:
                if connection_id.startswith(prefix):
                    to_remove.append(connection_id)

            for connection_id in to_remove:
                self.long_lived_connections.remove(connection_id)

            # 清理缓存
            keys_to_remove = []
            cache_prefix = f"{agent_id}:"
            for cache_key in self._service_cache:
                if cache_key.startswith(cache_prefix):
                    keys_to_remove.append(cache_key)

            for key in keys_to_remove:
                del self._service_cache[key]

            self._logger.info(f"Cleared all agent services: {agent_id}, service count: {len(services)}")

        except Exception as e:
            self._logger.error(f"Failed to clear agent services {agent_id}: {e}")

    async def clear_async(self, agent_id: str) -> None:
        """
        异步清除指定agent的所有服务

        Args:
            agent_id: Agent ID
        """
        # 简化实现：同步调用
        self.clear(agent_id)

    def _add_tools_to_service(self, agent_id: str, service_name: str,
                            tools: List[Tuple[str, Dict[str, Any]]]):
        """
        添加工具到服务

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            tools: 工具列表
        """
        try:
            self._logger.info(f"[ADD_TOOLS] Starting to add tools to service: agent={agent_id}, service={service_name}, tools_count={len(tools)}")
            service_global_name = self._naming.generate_service_global_name(service_name, agent_id)

            for tool_name, tool_def in tools:
                # 生成工具全局名称
                # NamingService.generate_tool_global_name 接受 (service_global_name, tool_original_name)
                tool_global_name = self._naming.generate_tool_global_name(service_global_name, tool_name)

                # 创建工具实体
                if self._tool_entity_manager:
                    self._sync_operation(
                        self._tool_entity_manager.create_tool(
                            service_global_name=service_global_name,
                            service_original_name=service_name,
                            source_agent=agent_id,
                            tool_original_name=tool_name,
                            tool_def=tool_def
                        ),
                        f"create_tool:{tool_name}"
                    )

                # 创建服务-工具关系
                # 方法签名：add_service_tool(service_global_name, service_original_name, source_agent, tool_global_name, tool_original_name)
                if self._relation_manager:
                    self._sync_operation(
                        self._relation_manager.add_service_tool(
                            service_global_name=service_global_name,
                            service_original_name=service_name,
                            source_agent=agent_id,
                            tool_global_name=tool_global_name,
                            tool_original_name=tool_name
                        ),
                        f"add_service_tool:{service_global_name}:{tool_global_name}"
                    )

        except Exception as e:
            self._logger.error(f"Failed to add tools to service {agent_id}:{service_name}: {e}")
            raise

    async def _add_tools_to_service_async(self, agent_id: str, service_name: str,
                                          tools: List[Tuple[str, Dict[str, Any]]]):
        """
        异步添加工具到服务

        遵循 "Functional Core, Imperative Shell" 架构原则：
        - 异步外壳直接使用 await 调用异步操作
        - 不通过 _sync_operation 转换

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            tools: 工具列表
        """
        try:
            self._logger.info(f"[ADD_TOOLS_ASYNC] Starting to add tools to service asynchronously: agent={agent_id}, service={service_name}, tools_count={len(tools)}")
            service_global_name = self._naming.generate_service_global_name(service_name, agent_id)

            for tool_name, tool_def in tools:
                # 生成工具全局名称
                tool_global_name = self._naming.generate_tool_global_name(service_global_name, tool_name)

                # 创建工具实体（异步）
                if self._tool_entity_manager:
                    await self._tool_entity_manager.create_tool(
                        service_global_name=service_global_name,
                        service_original_name=service_name,
                        source_agent=agent_id,
                        tool_original_name=tool_name,
                        tool_def=tool_def
                    )

                # 创建服务-工具关系（异步）
                if self._relation_manager:
                    await self._relation_manager.add_service_tool(
                        service_global_name=service_global_name,
                        service_original_name=service_name,
                        source_agent=agent_id,
                        tool_global_name=tool_global_name,
                        tool_original_name=tool_name
                    )

        except Exception as e:
            self._logger.error(f"Failed to add tools to service asynchronously {agent_id}:{service_name}: {e}")
            raise

    def _sync_operation(self, async_coro, operation_name: str = "同步操作"):
        """
        执行同步操作

        Args:
            async_coro: 异步操作
            operation_name: 操作名称

        Returns:
            异步操作结果
        """
        try:
            if self._cache_manager:
                # 使用 async_to_sync 方法执行异步协程
                return self._cache_manager.async_to_sync(async_coro, operation_name)
            else:
                # 直接执行异步操作
                import asyncio
                try:
                    loop = asyncio.get_event_loop()
                    if loop.is_running():
                        # 在新线程中运行
                        import concurrent.futures

                        def run_in_thread():
                            new_loop = asyncio.new_event_loop()
                            asyncio.set_event_loop(new_loop)
                            try:
                                return new_loop.run_until_complete(async_coro)
                            finally:
                                new_loop.close()

                        with concurrent.futures.ThreadPoolExecutor() as executor:
                            future = executor.submit(run_in_thread)
                            return future.result()
                    else:
                        return loop.run_until_complete(async_coro)
                except RuntimeError:
                    loop = asyncio.new_event_loop()
                    asyncio.set_event_loop(loop)
                    try:
                        return loop.run_until_complete(async_coro)
                    finally:
                        loop.close()

        except Exception as e:
            self._logger.error(f"Sync operation failed {operation_name}: {e}")
            raise

    def get_service_stats(self, agent_id: Optional[str] = None) -> Dict[str, Any]:
        """
        获取服务统计信息

        Args:
            agent_id: 可选的agent_id过滤

        Returns:
            统计信息字典
        """
        try:
            if agent_id:
                # 获取指定agent的统计
                services = self.get_services_for_agent(agent_id)
                long_lived_count = len(self.get_long_lived_services(agent_id))

                return {
                    "agent_id": agent_id,
                    "total_services": len(services),
                    "long_lived_services": long_lived_count,
                    "namespace": self._namespace
                }
            else:
                # 获取全局统计
                all_agents = set()
                for cache_key in self._service_cache:
                    agent_id = cache_key.split(":", 1)[0]
                    all_agents.add(agent_id)

                return {
                    "total_agents": len(all_agents),
                    "total_services": len(self._service_cache),
                    "long_lived_connections": len(self.long_lived_connections),
                    "namespace": self._namespace
                }

        except Exception as e:
            self._logger.error(f"Failed to get service statistics: {e}")
            return {
                "error": str(e),
                "namespace": self._namespace
            }

    def get_service_summary_async(self, agent_id: str, service_name: str) -> Dict[str, Any]:
        """
        异步获取服务摘要信息

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务摘要信息
        """
        # 简化实现：同步调用
        return self.get_service_summary(agent_id, service_name)

    def get_service_summary(self, agent_id: str, service_name: str) -> Dict[str, Any]:
        """
        获取服务摘要信息

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务摘要信息
        """
        try:
            details = self.get_service_details(agent_id, service_name)
            if not details:
                return {}

            # 构建摘要信息
            summary = {
                "agent_id": agent_id,
                "service_name": service_name,
                "global_name": details.get("global_name"),
                "state": details.get("state"),
                "has_session": details.get("has_session", False),
                "tools_count": len(details.get("tools", [])),
                "is_long_lived": details.get("is_long_lived", False),
                "config": details.get("config", {}),
                "last_updated": datetime.now().isoformat()
            }

            return summary

        except Exception as e:
            self._logger.error(f"Failed to get service summary {agent_id}:{service_name}: {e}")
            return {}

    def get_complete_service_info_async(self, agent_id: str, service_name: str) -> Dict[str, Any]:
        """
        异步获取完整服务信息

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            完整服务信息
        """
        # 简化实现：同步调用
        return self.get_complete_service_info(agent_id, service_name)

    def get_complete_service_info(self, agent_id: str, service_name: str) -> Dict[str, Any]:
        """
        获取完整服务信息

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            完整服务信息
        """
        try:
            # 获取基础服务详情
            details = self.get_service_details(agent_id, service_name)
            if not details:
                return {}

            # 获取工具详细信息
            tools_info = []
            if self._tool_manager:
                tools = self._tool_manager.get_tools_for_service(agent_id, service_name)
                for tool_name in tools:
                    tool_info = self._tool_manager.get_tool_info(agent_id, tool_name)
                    if tool_info:
                        tools_info.append(tool_info)

            # 获取状态信息
            # 注意：get_complete_service_info 是同步方法，但 get_service_metadata_async 是异步方法
            # 这里使用内存缓存中的元数据，不从 pykv 读取
            # 如需从 pykv 读取，请使用 get_complete_service_info_async 异步方法
            state_info = {}
            if self._state_manager:
                state = self._state_manager.get_service_state(agent_id, service_name)
                # 从内存缓存获取元数据（同步方法中不能调用异步方法）
                cache_key = f"{agent_id}:{service_name}"
                metadata = self._state_manager._metadata_cache.get(cache_key)
                state_info = {
                    "state": state,
                    "metadata": metadata
                }

            # 获取会话信息
            session_info = {}
            if self._session_manager:
                session = self._session_manager.get_session(agent_id, service_name)
                session_info = {
                    "has_session": session is not None,
                    "session_type": type(session).__name__ if session else None
                }

            # 获取摘要信息
            summary = self.get_service_summary(agent_id, service_name)

            # 构建完整信息
            complete_info = {
                **details,
                "tools": tools_info,
                "tool_count": len(tools_info),  # 添加 tool_count 字段
                "state_info": state_info,
                "session_info": session_info,
                "summary": summary
            }

            return complete_info

        except Exception as e:
            self._logger.error(f"Failed to get complete service info {agent_id}:{service_name}: {e}")
            return {}

    def get_all_services_complete_info(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        获取所有服务的完整信息

        Args:
            agent_id: Agent ID

        Returns:
            所有服务的完整信息列表
        """
        try:
            services = self.get_services_for_agent(agent_id)
            all_info = []

            for service_name in services:
                complete_info = self.get_complete_service_info(agent_id, service_name)
                if complete_info:
                    all_info.append(complete_info)

            return all_info

        except Exception as e:
            self._logger.error(f"Failed to get all services complete info {agent_id}: {e}")
            return []

    def get_services_by_state(self, agent_id: str, states: List['ServiceConnectionState']) -> List[str]:
        """
        根据状态获取服务列表

        Args:
            agent_id: Agent ID
            states: 状态列表

        Returns:
            符合状态的服务名称列表
        """
        try:
            if self._state_manager:
                return self._state_manager.get_services_by_state(agent_id, states)
            else:
                # 从缓存获取
                matching_services = []
                for service_name in self.get_services_for_agent(agent_id):
                    details = self.get_service_details(agent_id, service_name)
                    if details.get("state") in states:
                        matching_services.append(service_name)
                return matching_services

        except Exception as e:
            self._logger.error(f"Failed to get services by status {agent_id}: {e}")
            return []

    def get_healthy_services(self, agent_id: str) -> List[str]:
        """
        获取健康服务列表

        Args:
            agent_id: Agent ID

        Returns:
            健康服务名称列表
        """
        try:
            from mcpstore.core.models.service import ServiceConnectionState
            return self.get_services_by_state(agent_id, [ServiceConnectionState.HEALTHY])

        except Exception as e:
            self._logger.error(f"Failed to get healthy services {agent_id}: {e}")
            return []

    def get_failed_services(self, agent_id: str) -> List[str]:
        """
        获取失败服务列表

        Args:
            agent_id: Agent ID

        Returns:
            失败服务名称列表
        """
        try:
            from mcpstore.core.models.service import ServiceConnectionState
            return self.get_services_by_state(agent_id, [
                ServiceConnectionState.CIRCUIT_OPEN,
                ServiceConnectionState.DISCONNECTED
            ])

        except Exception as e:
            self._logger.error(f"Failed to get failed services {agent_id}: {e}")
            return []

    def get_services_with_tools(self, agent_id: str) -> List[str]:
        """
        获取有工具的服务列表

        Args:
            agent_id: Agent ID

        Returns:
            有工具的服务名称列表
        """
        try:
            services_with_tools = []
            services = self.get_services_for_agent(agent_id)

            for service_name in services:
                if self._tool_manager:
                    tools = self._tool_manager.get_tools_for_service(agent_id, service_name)
                    if tools:
                        services_with_tools.append(service_name)
                else:
                    # 从缓存检查
                    details = self.get_service_details(agent_id, service_name)
                    if details.get("tools"):
                        services_with_tools.append(service_name)

            return services_with_tools

        except Exception as e:
            self._logger.error(f"Failed to get services with tools {agent_id}: {e}")
            return []

    def should_cache_aggressively(self, agent_id: str, service_name: str) -> bool:
        """
        判断是否应该积极缓存

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            是否应该积极缓存
        """
        try:
            # 检查是否为长生命周期服务
            if self.is_long_lived_service(agent_id, service_name):
                return True

            # 检查服务是否有很多工具
            if self._tool_manager:
                tools = self._tool_manager.get_tools_for_service(agent_id, service_name)
                if len(tools) > 5:  # 工具数量超过5个
                    return True

            return False

        except Exception as e:
            self._logger.error(f"Failed to determine cache strategy {agent_id}:{service_name}: {e}")
            return False

    def remove_service_lifecycle_data(self, agent_id: str, service_name: str):
        """
        移除服务的生命周期数据

        Args:
            agent_id: Agent ID
            service_name: 服务名称
        """
        try:
            # 从长生命周期连接中移除
            connection_id = f"{agent_id}:{service_name}"
            if connection_id in self.long_lived_connections:
                self.long_lived_connections.remove(connection_id)

            # 从服务缓存中移除
            cache_key = f"{agent_id}:{service_name}"
            if cache_key in self._service_cache:
                del self._service_cache[cache_key]

            self._logger.debug(f"Removing service lifecycle data: {cache_key}")

        except Exception as e:
            self._logger.error(f"Failed to remove service lifecycle data {agent_id}:{service_name}: {e}")

    def set_service_lifecycle_data(self, agent_id: str, service_name: str, data: Dict[str, Any]):
        """
        设置服务的生命周期数据

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            data: 生命周期数据
        """
        try:
            cache_key = f"{agent_id}:{service_name}"

            # 确保缓存中存在基础信息
            if cache_key not in self._service_cache:
                self._service_cache[cache_key] = {}

            # 更新生命周期数据
            if "lifecycle_data" not in self._service_cache[cache_key]:
                self._service_cache[cache_key]["lifecycle_data"] = {}

            self._service_cache[cache_key]["lifecycle_data"].update(data)
            self._service_cache[cache_key]["lifecycle_data"]["last_updated"] = datetime.now().isoformat()

            self._logger.debug(f"Setting service lifecycle data: {cache_key}")

        except Exception as e:
            self._logger.error(f"Failed to set service lifecycle data {agent_id}:{service_name}: {e}")

    def clear_agent_lifecycle_data(self, agent_id: str):
        """
        清除agent的所有生命周期数据

        Args:
            agent_id: Agent ID
        """
        try:
            # 清理所有服务的生命周期数据
            services = self.get_services_for_agent(agent_id)
            for service_name in services:
                self.remove_service_lifecycle_data(agent_id, service_name)

            # 清理长生命周期连接
            prefix = f"{agent_id}:"
            to_remove = []
            for connection_id in self.long_lived_connections:
                if connection_id.startswith(prefix):
                    to_remove.append(connection_id)

            for connection_id in to_remove:
                self.long_lived_connections.remove(connection_id)

            self._logger.info(f"Cleared agent lifecycle data: {agent_id}")

        except Exception as e:
            self._logger.error(f"Failed to clear agent lifecycle data {agent_id}: {e}")

    def get_stats(self) -> Dict[str, Any]:
        """
        获取服务管理器的统计信息

        Returns:
            统计信息字典
        """
        return {
            "namespace": self._namespace,
            "service_cache_size": len(self._service_cache),
            "long_lived_connections": len(self.long_lived_connections),
            "has_service_entity_manager": self._service_entity_manager is not None,
            "has_relation_manager": self._relation_manager is not None,
            "has_tool_manager": self._tool_manager is not None,
            "has_state_manager": self._state_manager is not None,
            "has_session_manager": self._session_manager is not None,
            "has_cache_manager": self._cache_manager is not None,
            "has_mapping_manager": self._mapping_manager is not None
        }

    # ==================== 客户端映射相关方法 ====================

    def get_service_client_id_async(self, agent_id: str, service_name: str) -> Optional[str]:
        """
        异步获取服务客户端ID

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            客户端ID或None
        """
        if self._mapping_manager:
            return self._mapping_manager.get_service_client_id_async(agent_id, service_name)
        return None

    def get_service_client_id(self, agent_id: str, service_name: str) -> Optional[str]:
        """
        获取服务客户端ID

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            客户端ID或None
        """
        if self._mapping_manager:
            return self._mapping_manager.get_service_client_id(agent_id, service_name)
        return None

    async def get_agent_clients_async(self, agent_id: str) -> List[str]:
        """
        从 pykv 关系层获取 Agent 的所有客户端
        
        [pykv 唯一真相源] 所有数据必须从 pykv 读取

        Args:
            agent_id: Agent ID

        Returns:
            客户端ID列表
            
        Raises:
            RuntimeError: 如果 mapping_manager 未初始化
        """
        if not self._mapping_manager:
            raise RuntimeError("MappingManager not initialized")
        return await self._mapping_manager.get_agent_clients_async(agent_id)

    def get_client_config_from_cache(self, client_id: str) -> Optional[Dict[str, Any]]:
        """
        从缓存获取客户端配置

        Args:
            client_id: 客户端ID

        Returns:
            客户端配置或None
        """
        if self._mapping_manager:
            return self._mapping_manager.get_client_config_from_cache(client_id)
        return None

    def add_client_config(self, client_id: str, config: Dict[str, Any]) -> None:
        """
        添加客户端配置

        Args:
            client_id: 客户端ID
            config: 客户端配置
        """
        if self._mapping_manager:
            self._mapping_manager.add_client_config(client_id, config)

    def set_service_client_mapping(self, agent_id: str, service_name: str, client_id: str) -> None:
        """
        设置服务客户端映射

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            client_id: 客户端ID
        """
        if self._mapping_manager:
            self._mapping_manager.set_service_client_mapping(agent_id, service_name, client_id)

    def remove_service_client_mapping(self, agent_id: str, service_name: str) -> None:
        """
        移除服务客户端映射

        Args:
            agent_id: Agent ID
            service_name: 服务名称
        """
        if self._mapping_manager:
            self._mapping_manager.remove_service_client_mapping(agent_id, service_name)

    def set_service_client_mapping_async(self, agent_id: str, service_name: str, client_id: str) -> None:
        """
        异步设置服务客户端映射

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            client_id: 客户端ID
        """
        if self._mapping_manager:
            self._mapping_manager.set_service_client_mapping_async(agent_id, service_name, client_id)

    def delete_service_client_mapping_async(self, agent_id: str, service_name: str) -> None:
        """
        异步删除服务客户端映射

        Args:
            agent_id: Agent ID
            service_name: 服务名称
        """
        if self._mapping_manager:
            self._mapping_manager.delete_service_client_mapping_async(agent_id, service_name)

    def add_agent_service_mapping(self, agent_id: str, local_name: str, global_name: str) -> None:
        """
        添加Agent服务映射

        Args:
            agent_id: Agent ID
            local_name: 本地服务名称
            global_name: 全局服务名称
        """
        if self._mapping_manager:
            self._mapping_manager.add_agent_service_mapping(agent_id, local_name, global_name)

    def get_global_name_from_agent_service(self, agent_id: str, local_name: str) -> Optional[str]:
        """
        从Agent服务获取全局名称

        Args:
            agent_id: Agent ID
            local_name: 本地服务名称

        Returns:
            全局名称或None
        """
        if self._mapping_manager:
            return self._mapping_manager.get_global_name_from_agent_service(agent_id, local_name)
        return None

    def get_global_name_from_agent_service_async(self, agent_id: str, local_name: str) -> Optional[str]:
        """
        异步从Agent服务获取全局名称

        Args:
            agent_id: Agent ID
            local_name: 本地服务名称

        Returns:
            全局名称或None
        """
        if self._mapping_manager:
            return self._mapping_manager.get_global_name_from_agent_service_async(agent_id, local_name)
        return None

    def get_agent_service_from_global_name(self, global_name: str) -> Optional[Tuple[str, str]]:
        """
        从全局名称获取Agent服务

        Args:
            global_name: 全局名称

        Returns:
            (agent_id, local_name) 元组或None
        """
        if self._mapping_manager:
            return self._mapping_manager.get_agent_service_from_global_name(global_name)
        return None

    def get_agent_services(self, agent_id: str) -> List[str]:
        """
        获取Agent的所有服务

        Args:
            agent_id: Agent ID

        Returns:
            服务名称列表
        """
        if self._mapping_manager:
            return self._mapping_manager.get_agent_services(agent_id)
        return []

    def is_agent_service(self, global_name: str) -> bool:
        """
        检查是否为Agent服务

        Args:
            global_name: 全局名称

        Returns:
            是否为Agent服务
        """
        if self._mapping_manager:
            return self._mapping_manager.is_agent_service(global_name)
        return False

    def remove_agent_service_mapping(self, agent_id: str, local_name: str) -> None:
        """
        移除Agent服务映射

        Args:
            agent_id: Agent ID
            local_name: 本地服务名称
        """
        if self._mapping_manager:
            self._mapping_manager.remove_agent_service_mapping(agent_id, local_name)

    def clear_agent_mappings(self, agent_id: str) -> None:
        """
        清除Agent的所有映射

        Args:
            agent_id: Agent ID
        """
        if self._mapping_manager:
            self._mapping_manager.clear_agent_mappings(agent_id)

    def clear_all_mappings(self) -> None:
        """
        清除所有映射
        """
        if self._mapping_manager:
            self._mapping_manager.clear_all_mappings()

    def get_mapping_stats(self) -> Dict[str, Any]:
        """
        获取映射统计信息

        Returns:
            统计信息字典
        """
        if self._mapping_manager:
            return self._mapping_manager.get_mapping_stats()
        return {}
