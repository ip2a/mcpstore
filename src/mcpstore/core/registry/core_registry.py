import os
import sys

sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import logging
from datetime import datetime
from typing import Dict, Any, Optional, Tuple, List, Set, TypeVar, Protocol

from ..models.service import ServiceConnectionState, ServiceStateMetadata
from .types import SessionProtocol, SessionType
from typing import TYPE_CHECKING
from .cache_backend import CacheBackend
from .memory_backend import MemoryCacheBackend

from .atomic import atomic_write

logger = logging.getLogger(__name__)

class ServiceRegistry:
    """
    Manages the state of connected services and their tools, with agent_id isolation.

    agent_id as primary key, implementing complete isolation between store/agent/agent:
    - self.sessions: Dict[agent_id, Dict[service_name, session]]
    - self.tool_cache: Dict[agent_id, Dict[tool_name, tool_def]]
    - self.tool_to_session_map: Dict[agent_id, Dict[tool_name, session]]
    - self.service_states: Dict[agent_id, Dict[service_name, ServiceConnectionState]]
    - self.service_metadata: Dict[agent_id, Dict[service_name, ServiceStateMetadata]]
    - self.agent_clients: Dict[agent_id, List[client_id]]
    - self.client_configs: Dict[client_id, config]
    - self.service_to_client: Dict[agent_id, Dict[service_name, client_id]]
    All operations must include agent_id, store level uses global_agent_store, agent level uses actual agent_id.
    """
    def __init__(self):
        # agent_id -> {service_name: session}
        self.sessions: Dict[str, Dict[str, Any]] = {}
        # Service health status management has been moved to ServiceLifecycleManager
        # agent_id -> {tool_name: tool_definition}
        self.tool_cache: Dict[str, Dict[str, Dict[str, Any]]] = {}
        # agent_id -> {tool_name: session}
        self.tool_to_session_map: Dict[str, Dict[str, Any]] = {}
        # agent_id -> {tool_name: service_name} (hard mapping)
        self.tool_to_service: Dict[str, Dict[str, str]] = {}
        # 长连接服务标记 - agent_id:service_name
        self.long_lived_connections: Set[str] = set()

        # 新增：生命周期状态支持
        # agent_id -> {service_name: ServiceConnectionState}
        self.service_states: Dict[str, Dict[str, ServiceConnectionState]] = {}
        # agent_id -> {service_name: ServiceStateMetadata}
        self.service_metadata: Dict[str, Dict[str, ServiceStateMetadata]] = {}

        #  新增：Agent-Client 映射缓存
        self.agent_clients: Dict[str, List[str]] = {}
        # 结构：{agent_id: [client_id1, client_id2, ...]}

        #  新增：Client 配置缓存
        self.client_configs: Dict[str, Dict[str, Any]] = {}
        # 结构：{client_id: {"mcpServers": {...}}}

        #  新增：Service 到 Client 的反向映射
        self.service_to_client: Dict[str, Dict[str, str]] = {}
        # 结构：{agent_id: {service_name: client_id}}

        #  新增：缓存同步状态
        from datetime import datetime
        self.cache_sync_status: Dict[str, datetime] = {}

        #  新增：Agent 服务映射关系
        # agent_id -> {local_name: global_name}
        self.agent_to_global_mappings: Dict[str, Dict[str, str]] = {}
        # global_name -> (agent_id, local_name)
        self.global_to_agent_mappings: Dict[str, Tuple[str, str]] = {}

        #  新增：状态同步管理器（延迟初始化）
        self._state_sync_manager = None

        # === Snapshot (A+B+D): immutable bundle and versioning ===
        # 当前有效的快照包（不可变结构）；读路径只读此指针，发布通过原子指针交换
        self._tools_snapshot_bundle: Optional[Dict[str, Any]] = None
        self._tools_snapshot_version: int = 0

        logger.debug("ServiceRegistry initialized with multi-context isolation")

        # Inject default cache backend (Memory); can be replaced with RedisBackend later
        self.cache_backend: CacheBackend = MemoryCacheBackend(self)


    def set_cache_backend(self, backend: CacheBackend) -> None:
        """Replace the cache backend implementation at runtime.
        Callers must ensure appropriate migration if switching from Memory to Redis.
        """
        self.cache_backend = backend

    def configure_cache_backend(self, config: Optional[Dict[str, Any]]) -> None:
        """Configure cache backend from a config dict without changing defaults.
        - If config is None or backend != 'redis', remains Memory.
        - If backend == 'redis', builds RedisCacheBackend via backend_factory and attaches provided client when present.
        This method performs no external I/O and does not install dependencies.
        """
        try:
            from .backend_factory import make_cache_backend
            backend = make_cache_backend(config, self)
            self.set_cache_backend(backend)
        except Exception as e:
            logger.error(f"Failed to configure cache backend, fallback to Memory. err={e}")

    def list_tools(self, agent_id: str) -> List[Dict[str, Any]]:
        """Return a list-like snapshot of tools for the given agent_id.

        The registry stores raw tool definitions; this method converts them
        into a minimal, stable structure compatible with ToolInfo fields.
        We avoid importing pydantic models here to keep registry free of heavy deps.
        """
        tools_map = self.tool_cache.get(agent_id, {})
        result: List[Dict[str, Any]] = []
        for tool_name, tool_def in tools_map.items():
            try:
                if isinstance(tool_def, dict) and "function" in tool_def:
                    fn = tool_def["function"]
                    result.append({
                        "name": fn.get("name", tool_name),
                        "description": fn.get("description", ""),
                        "service_name": fn.get("service_name", ""),
                        "client_id": None,
                        "inputSchema": fn.get("parameters")
                    })
                else:
                    # Fallback best-effort mapping
                    result.append({
                        "name": tool_name,
                        "description": str(tool_def.get("description", "")) if isinstance(tool_def, dict) else "",
                        "service_name": tool_def.get("service_name", "") if isinstance(tool_def, dict) else "",
                        "client_id": None,
                        "inputSchema": tool_def.get("parameters") if isinstance(tool_def, dict) else None
                    })
            except Exception as e:
                logger.warning(f"[REGISTRY] Failed to map tool '{tool_name}': {e}")
        return result

    # === Snapshot building and publishing API ===
    def get_tools_snapshot_bundle(self) -> Optional[Dict[str, Any]]:
        """
        返回当前已发布的工具快照包（只读指针）。
        结构（示例）：
        {
            "tools": {
                "services": { "weather": [ToolItem, ...], ... },
                "tools_by_fullname": { "weather_get": ToolItem, ... }
            },
            "mappings": {
                "agent_to_global": { agent_id: { local: global } },
                "global_to_agent": { global: (agent_id, local) }
            },
            "meta": { "version": int, "created_at": float }
        }
        """
        return self._tools_snapshot_bundle

    def rebuild_tools_snapshot(self, global_agent_id: str) -> Dict[str, Any]:
        """
        重建不可变的工具快照包，并使用原子指针交换发布（Copy-On-Write）。
        仅依据 global_agent_id 下的缓存构建全局真源快照；Agent 视图由上层基于映射做投影。
        """
        from time import time

        # 构建全局工具索引
        services_index: Dict[str, List[Dict[str, Any]]] = {}
        tools_by_fullname: Dict[str, Dict[str, Any]] = {}

        # 遍历 global_agent_id 下的所有服务名
        service_names = self.get_all_service_names(global_agent_id)
        for service_name in service_names:
            # 获取该服务的工具名列表
            tool_names = self.get_tools_for_service(global_agent_id, service_name)
            if not tool_names:
                services_index[service_name] = []
                continue

            items: List[Dict[str, Any]] = []
            for tool_name in tool_names:
                info = self.get_tool_info(global_agent_id, tool_name)
                if not info:
                    continue
                # 规范化为快照条目
                # name: 使用 display_name 作为对外“展示名”；original_name 保留原始名称（如有）
                item = {
                    "name": info.get("display_name", info.get("name", tool_name)),
                    "description": info.get("description", ""),
                    "service_name": service_name,
                    "client_id": info.get("client_id"),
                    "inputSchema": info.get("inputSchema", {}),
                    "original_name": info.get("original_name", info.get("name", tool_name))
                }
                items.append(item)
                tools_by_fullname[info.get("name", tool_name)] = item
            services_index[service_name] = items

        # 复制映射快照（只读）
        agent_to_global = {aid: dict(mapping) for aid, mapping in self.agent_to_global_mappings.items()}
        global_to_agent = dict(self.global_to_agent_mappings)

        new_bundle: Dict[str, Any] = {
            "tools": {
                "services": services_index,
                "tools_by_fullname": tools_by_fullname
            },
            "mappings": {
                "agent_to_global": agent_to_global,
                "global_to_agent": global_to_agent
            },
            "meta": {
                "version": self._tools_snapshot_version + 1,
                "created_at": time()
            }
        }

        # 原子发布（指针交换）
        self._tools_snapshot_bundle = new_bundle
        self._tools_snapshot_version += 1
        logger.debug(f"Tools bundle published: v{self._tools_snapshot_version}, services={len(services_index)}")
        return new_bundle

    def _ensure_state_sync_manager(self):
        """确保状态同步管理器已初始化"""
        if self._state_sync_manager is None:
            from mcpstore.core.sync.shared_client_state_sync import SharedClientStateSyncManager
            self._state_sync_manager = SharedClientStateSyncManager(self)
            logger.debug("[REGISTRY] state_sync_manager initialized")

    def clear(self, agent_id: str):
        """
        清空指定 agent_id 的所有注册服务和工具。
        只影响该 agent_id 下的服务、工具、会话，不影响其它 agent。
        """
        self.sessions.pop(agent_id, None)
        self.tool_cache.pop(agent_id, None)
        self.tool_to_session_map.pop(agent_id, None)
        self.tool_to_service.pop(agent_id, None)

        #  清理新增的缓存字段
        self.service_states.pop(agent_id, None)
        self.service_metadata.pop(agent_id, None)
        self.service_to_client.pop(agent_id, None)

        # 清理Agent-Client映射和相关Client配置
        client_ids = self.agent_clients.pop(agent_id, [])
        for client_id in client_ids:
            # 检查client是否被其他agent使用
            is_used_by_others = any(
                client_id in clients for other_agent, clients in self.agent_clients.items()
                if other_agent != agent_id
            )
            if not is_used_by_others:
                self.client_configs.pop(client_id, None)

    @atomic_write(agent_id_param="agent_id", use_lock=True)
    def add_service(self, agent_id: str, name: str, session: Any = None, tools: List[Tuple[str, Dict[str, Any]]] = None,
                    service_config: Dict[str, Any] = None, state: 'ServiceConnectionState' = None,
                    preserve_mappings: bool = False) -> List[str]:
        """
        为指定 agent_id 注册服务及其工具（支持所有状态的服务）
        - agent_id: store/agent 的唯一标识
        - name: 服务名
        - session: 服务会话对象（可选，失败的服务为None）
        - tools: [(tool_name, tool_def)]（可选，失败的服务为空列表）
        - service_config: 服务配置信息
        - state: 服务状态（可选，如果不提供则根据session判断）
        - preserve_mappings: 是否保留现有的Agent-Client映射关系（优雅修复用）
        返回实际注册的工具名列表。
        """
        #  新增：支持所有状态的服务注册
        tools = tools or []
        service_config = service_config or {}

        # 初始化数据结构
        if agent_id not in self.sessions:
            self.sessions[agent_id] = {}
        if agent_id not in self.tool_cache:
            self.tool_cache[agent_id] = {}
        if agent_id not in self.tool_to_session_map:
            self.tool_to_session_map[agent_id] = {}
        if agent_id not in self.tool_to_service:
            self.tool_to_service[agent_id] = {}
        if agent_id not in self.service_states:
            self.service_states[agent_id] = {}
        if agent_id not in self.service_metadata:
            self.service_metadata[agent_id] = {}

        # 确定服务状态
        if state is None:
            if session is not None and len(tools) > 0:
                from mcpstore.core.models.service import ServiceConnectionState
                state = ServiceConnectionState.HEALTHY
            elif session is not None:
                from mcpstore.core.models.service import ServiceConnectionState
                state = ServiceConnectionState.WARNING  # 有连接但无工具
            else:
                from mcpstore.core.models.service import ServiceConnectionState
                state = ServiceConnectionState.DISCONNECTED  # 连接失败

        #  优雅修复：智能处理现有服务
        if name in self.sessions[agent_id]:
            if preserve_mappings:
                # 保留映射关系，只清理工具缓存
                logger.debug(f"[ADD_SERVICE] exists keep_mappings=True clear_tools_only name={name}")
                self.clear_service_tools_only(agent_id, name)
            else:
                # 传统逻辑：完全移除服务
                logger.warning(f"Attempting to add already registered service: {name} for agent {agent_id}. Removing old service before overwriting.")
                self.remove_service(agent_id, name)

        # 存储服务信息（即使连接失败也存储）
        self.sessions[agent_id][name] = session  # 失败的服务session为None
        self.service_states[agent_id][name] = state

        # 关键：存储完整的服务配置和元数据
        if name not in self.service_metadata[agent_id]:
            from mcpstore.core.models.service import ServiceStateMetadata
            from datetime import datetime
            self.service_metadata[agent_id][name] = ServiceStateMetadata(
                service_name=name,
                agent_id=agent_id,
                state_entered_time=datetime.now(),
                service_config=service_config,  #  存储完整配置
                consecutive_failures=0 if session else 1,
                error_message=None if session else "Connection failed"
            )

        added_tool_names = []
        for tool_name, tool_definition in tools:
            # 🆕 使用新的工具归属判断逻辑
            # 检查工具定义中的服务归属
            tool_service_name = None
            if "function" in tool_definition:
                tool_service_name = tool_definition["function"].get("service_name")
            else:
                tool_service_name = tool_definition.get("service_name")

            # 验证工具是否属于当前服务
            if tool_service_name and tool_service_name != name:
                logger.warning(f"Tool '{tool_name}' belongs to service '{tool_service_name}', not '{name}'. Skipping this tool.")
                continue

            # 检查工具名冲突
            if tool_name in self.tool_cache[agent_id]:
                existing_session = self.tool_to_session_map[agent_id].get(tool_name)
                if existing_session is not session:
                    logger.warning(f"Tool name conflict: '{tool_name}' from {name} for agent {agent_id} conflicts with existing tool. Skipping this tool.")
                    continue

            # 存储工具 + 硬映射（并同步后端定义以备将来切换 Redis）
            self.tool_cache[agent_id][tool_name] = tool_definition
            self.tool_to_session_map[agent_id][tool_name] = session
            self.cache_backend.map_tool_to_service(agent_id, tool_name, name)
            # 新增：同步工具定义至后端（Memory 为内存写，Redis 为JSON写）
            try:
                self.cache_backend.upsert_tool_def(agent_id, tool_name, tool_definition)
            except Exception as e:
                logger.debug(f"upsert_tool_def failed: agent_id={agent_id} tool={tool_name} service={name} err={e}")
            added_tool_names.append(tool_name)

        logger.debug(f"Service added: {name} ({state.value}, {len(tools)} tools) for agent {agent_id}")
        return added_tool_names

    def add_failed_service(self, agent_id: str, name: str, service_config: Dict[str, Any],
                          error_message: str, state: 'ServiceConnectionState' = None):
        """
        注册失败的服务到缓存
        """
        if state is None:
            from mcpstore.core.models.service import ServiceConnectionState
            state = ServiceConnectionState.DISCONNECTED

        added_tools = self.add_service(
            agent_id=agent_id,
            name=name,
            session=None,
            tools=[],
            service_config=service_config,
            state=state
        )

        # 更新错误信息
        if agent_id in self.service_metadata and name in self.service_metadata[agent_id]:
            self.service_metadata[agent_id][name].error_message = error_message

        return added_tools

    @atomic_write(agent_id_param="agent_id", use_lock=True)

    def remove_service(self, agent_id: str, name: str) -> Optional[Any]:
        """
        移除指定 agent_id 下的服务及其所有工具。
        只影响该 agent_id，不影响其它 agent。
        """
        session = self.sessions.get(agent_id, {}).pop(name, None)
        if not session:
            logger.warning(f"Attempted to remove non-existent service: {name} for agent {agent_id}")
            # 即使session不存在，也要清理可能存在的缓存数据
            self._cleanup_service_cache_data(agent_id, name)
            return None

        # Remove associated tools efficiently
        tools_to_remove = [tool_name for tool_name, owner_session in self.tool_to_session_map.get(agent_id, {}).items() if owner_session is session]
        for tool_name in tools_to_remove:
            if tool_name in self.tool_cache.get(agent_id, {}): del self.tool_cache[agent_id][tool_name]
            if tool_name in self.tool_to_session_map.get(agent_id, {}): del self.tool_to_session_map[agent_id][tool_name]
            self.cache_backend.unmap_tool(agent_id, tool_name)
            #
            try:
                self.cache_backend.delete_tool_def(agent_id, tool_name)
            except Exception as e:
                logger.debug(f"delete_tool_def failed: agent_id={agent_id} tool={tool_name} service={name} err={e}")

        #  清理新增的缓存字段
        self._cleanup_service_cache_data(agent_id, name)

        logger.debug(f"Service removed: {name} for agent {agent_id}")
        return session

    @atomic_write(agent_id_param="agent_id", use_lock=True)

    def clear_service_tools_only(self, agent_id: str, service_name: str):
        """
        只清理服务的工具缓存，保留Agent-Client映射关系

        这是优雅修复方案的核心方法：
        - 清理工具缓存和工具-会话映射
        - 保留Agent-Client映射
        - 保留Client配置
        - 保留Service-Client映射
        """
        try:
            # 获取现有会话
            existing_session = self.sessions.get(agent_id, {}).get(service_name)
            if not existing_session:
                logger.debug(f"[CLEAR_TOOLS] no_session service={service_name} skip=True")
                return

            # 只清理工具相关的缓存
            tools_to_remove = [
                tool_name for tool_name, owner_session
                in self.tool_to_session_map.get(agent_id, {}).items()
                if owner_session is existing_session
            ]

            for tool_name in tools_to_remove:
                # 清理工具缓存
                if agent_id in self.tool_cache and tool_name in self.tool_cache[agent_id]:
                    del self.tool_cache[agent_id][tool_name]
                # 清理工具-会话映射
                if agent_id in self.tool_to_session_map and tool_name in self.tool_to_session_map[agent_id]:
                    del self.tool_to_session_map[agent_id][tool_name]
                # 清理工具-服务硬映射
                self.cache_backend.unmap_tool(agent_id, tool_name)
                # 同步后端删除工具定义
                try:
                    self.cache_backend.delete_tool_def(agent_id, tool_name)
                except Exception as e:
                    logger.debug(f"delete_tool_def failed: agent_id={agent_id} tool={tool_name} service={service_name} err={e}")

            # 清理会话（会被新会话替换）
            if agent_id in self.sessions and service_name in self.sessions[agent_id]:
                del self.sessions[agent_id][service_name]

            logger.debug(f"[CLEAR_TOOLS] cleared_tools service={service_name} count={len(tools_to_remove)} keep_mappings=True")

        except Exception as e:
            logger.error(f"Failed to clear service tools for {service_name}: {e}")

    def _cleanup_service_cache_data(self, agent_id: str, service_name: str):
        """清理服务相关的缓存数据"""
        # 清理服务状态和元数据
        if agent_id in self.service_states:
            self.service_states[agent_id].pop(service_name, None)
        if agent_id in self.service_metadata:
            self.service_metadata[agent_id].pop(service_name, None)

        # 清理Service-Client映射
        client_id = self.get_service_client_id(agent_id, service_name)
        if client_id:
            self.remove_service_client_mapping(agent_id, service_name)

            # 检查client是否还有其他服务
            client_config = self.get_client_config_from_cache(client_id)
            if client_config:
                remaining_services = client_config.get("mcpServers", {})
                if service_name in remaining_services:
                    del remaining_services[service_name]

                # 如果client没有其他服务，移除client
                if not remaining_services:
                    self.remove_client_config(client_id)
                    self.remove_agent_client_mapping(agent_id, client_id)

    def get_session(self, agent_id: str, name: str) -> Optional[Any]:
        """
        获取指定 agent_id 下的服务会话。
        """
        return self.sessions.get(agent_id, {}).get(name)

    def get_session_for_tool(self, agent_id: str, tool_name: str) -> Optional[Any]:
        """
        获取指定 agent_id 下工具对应的服务会话。
        """
        return self.tool_to_session_map.get(agent_id, {}).get(tool_name)

    def get_all_tools(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        获取指定 agent_id 下所有工具的定义。
        """
        all_tools = []
        for tool_name, tool_def in self.tool_cache.get(agent_id, {}).items():
            session = self.tool_to_session_map.get(agent_id, {}).get(tool_name)
            service_name = None
            for name, sess in self.sessions.get(agent_id, {}).items():
                if sess is session:
                    service_name = name
                    break
            tool_with_service = tool_def.copy()
            if "function" not in tool_with_service and isinstance(tool_with_service, dict):
                tool_with_service = {
                    "type": "function",
                    "function": tool_with_service
                }
            if "function" in tool_with_service:
                function_data = tool_with_service["function"]
                if service_name:
                    original_description = function_data.get("description", "")
                    if not original_description.endswith(f" (来自服务: {service_name})"):
                        function_data["description"] = f"{original_description} (来自服务: {service_name})"
                function_data["service_info"] = {"service_name": service_name}
            all_tools.append(tool_with_service)
        logger.debug(f"Retrieved {len(all_tools)} tools from {len(self.get_all_service_names(agent_id))} services for agent {agent_id}")
        return all_tools

    def get_all_tool_info(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        获取指定 agent_id 下所有工具的详细信息。
        """
        tools_info = []
        for tool_name in self.tool_cache.get(agent_id, {}).keys():
            session = self.tool_to_session_map.get(agent_id, {}).get(tool_name)
            service_name = None
            for name, sess in self.sessions.get(agent_id, {}).items():
                if sess is session:
                    service_name = name
                    break
            detailed_tool = self._get_detailed_tool_info(agent_id, tool_name)
            if detailed_tool:
                detailed_tool["service_name"] = service_name
                tools_info.append(detailed_tool)
        return tools_info

    def get_connected_services(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        获取指定 agent_id 下所有已连接服务的信息。
        """
        services = []
        for name in self.get_all_service_names(agent_id):
            tools = self.get_tools_for_service(agent_id, name)
            services.append({
                "name": name,
                "tool_count": len(tools)
            })
        return services

    def get_tools_for_service(self, agent_id: str, name: str) -> List[str]:
        """
        获取指定 agent_id 下某服务的所有工具名。
         修复：改为从service_to_client映射和tool_cache获取，而不是依赖sessions
        """
        logger.info(f"[REGISTRY] get_tools service={name} agent_id={agent_id}")

        #  修复：首先检查服务是否存在
        if not self.has_service(agent_id, name):
            logger.warning(f"[REGISTRY] service_not_exists service={name}")
            return []

        #  优先：使用工具→服务硬映射
        tools = []
        tool_cache = self.tool_cache.get(agent_id, {})
        tool_to_session = self.tool_to_session_map.get(agent_id, {})
        tool_to_service = self.tool_to_service.get(agent_id, {})

        # 获取该服务的session（如果存在）
        service_session = self.sessions.get(agent_id, {}).get(name)

        logger.debug(f"[REGISTRY] tool_cache_size={len(tool_cache)} tool_to_session_size={len(tool_to_session)} tool_to_service_size={len(tool_to_service)}")

        for tool_name in tool_cache.keys():
            mapped_service = tool_to_service.get(tool_name)
            if mapped_service == name:
                tools.append(tool_name)
                continue
            # 次选：当硬映射缺失时，使用会话匹配（避免历史数据缺口）
            tool_session = tool_to_session.get(tool_name)
            if service_session and tool_session is service_session:
                tools.append(tool_name)

        logger.debug(f"[REGISTRY] found_tools service={name} count={len(tools)} list={tools}")
        return tools

    def _extract_description_from_schema(self, prop_info):
        """从 schema 中提取描述信息"""
        if isinstance(prop_info, dict):
            # 优先查找 description 字段
            if 'description' in prop_info:
                return prop_info['description']
            # 其次查找 title 字段
            elif 'title' in prop_info:
                return prop_info['title']
            # 检查是否有 anyOf 或 allOf 结构
            elif 'anyOf' in prop_info:
                for item in prop_info['anyOf']:
                    if isinstance(item, dict) and 'description' in item:
                        return item['description']
            elif 'allOf' in prop_info:
                for item in prop_info['allOf']:
                    if isinstance(item, dict) and 'description' in item:
                        return item['description']

        return "无描述"

    def _extract_type_from_schema(self, prop_info):
        """从 schema 中提取类型信息"""
        if isinstance(prop_info, dict):
            if 'type' in prop_info:
                return prop_info['type']
            elif 'anyOf' in prop_info:
                # 处理 Union 类型
                types = []
                for item in prop_info['anyOf']:
                    if isinstance(item, dict) and 'type' in item:
                        types.append(item['type'])
                return '|'.join(types) if types else '未知'
            elif 'allOf' in prop_info:
                # 处理 intersection 类型
                for item in prop_info['allOf']:
                    if isinstance(item, dict) and 'type' in item:
                        return item['type']

        return "未知"

    def get_tool_info(self, agent_id: str, tool_name: str) -> Dict[str, Any]:
        """
        获取指定 agent_id 下某工具的详细信息，返回格式化的工具信息。
        """
        tool_def = self.tool_cache.get(agent_id, {}).get(tool_name)
        if not tool_def:
            return None

        session = self.tool_to_session_map.get(agent_id, {}).get(tool_name)
        service_name = None
        if session:
            for name, sess in self.sessions.get(agent_id, {}).items():
                if sess is session:
                    service_name = name
                    break

        # 获取 Client ID
        client_id = self.get_service_client_id(agent_id, service_name) if service_name else None

        # 处理不同的工具定义格式
        if "function" in tool_def:
            function_data = tool_def["function"]
            return {
                'name': tool_name,
                'display_name': function_data.get('display_name', tool_name),
                'original_name': function_data.get('name', tool_name),
                'description': function_data.get('description', ''),
                'inputSchema': function_data.get('parameters', {}),
                'service_name': service_name,
                'client_id': client_id
            }
        else:
            return {
                'name': tool_name,
                'display_name': tool_def.get('display_name', tool_name),
                'original_name': tool_def.get('name', tool_name),
                'description': tool_def.get('description', ''),
                'inputSchema': tool_def.get('parameters', {}),
                'service_name': service_name,
                'client_id': client_id
            }

    def _get_detailed_tool_info(self, agent_id: str, tool_name: str) -> Dict[str, Any]:
        """
        获取指定 agent_id 下某工具的详细信息。
        """
        tool_def = self.tool_cache.get(agent_id, {}).get(tool_name)
        if not tool_def:
            return {}
        session = self.tool_to_session_map.get(agent_id, {}).get(tool_name)
        service_name = None
        if session:
            for name, sess in self.sessions.get(agent_id, {}).items():
                if sess is session:
                    service_name = name
                    break

        if "function" in tool_def:
            function_data = tool_def["function"]
            tool_info = {
                "name": tool_name,  # 这是存储的键名（显示名称）
                "display_name": function_data.get("display_name", tool_name),  # 用户友好的显示名称
                "description": function_data.get("description", ""),
                "service_name": service_name,
                "inputSchema": function_data.get("parameters", {}),
                "original_name": function_data.get("name", tool_name)  # FastMCP 原始名称
            }
        else:
            tool_info = {
                "name": tool_name,
                "display_name": tool_def.get("display_name", tool_name),
                "description": tool_def.get("description", ""),
                "service_name": service_name,
                "inputSchema": tool_def.get("parameters", {}),
                "original_name": tool_def.get("name", tool_name)
            }
        return tool_info

    def get_service_details(self, agent_id: str, name: str) -> Dict[str, Any]:
        """
        获取指定 agent_id 下某服务的详细信息。
        """
        if name not in self.sessions.get(agent_id, {}):
            return {}

        logger.info(f"Getting service details for: {name} (agent_id={agent_id})")
        session = self.sessions.get(agent_id, {}).get(name)

        # 只在调试特定问题时打印详细日志
        logger.debug(f"get_service_details: agent_id={agent_id}, name={name}, session_id={id(session) if session else None}")

        tools = self.get_tools_for_service(agent_id, name)
        # service_health已废弃，使用None作为默认值
        last_heartbeat = None
        detailed_tools = []
        for tool_name in tools:
            detailed_tool = self._get_detailed_tool_info(agent_id, tool_name)
            if detailed_tool:
                detailed_tools.append(detailed_tool)
        # TODO: 添加Resources和Prompts信息收集
        # 当前版本暂时返回空值，后续版本将实现完整的资源和提示词统计

        return {
            "name": name,
            "tools": detailed_tools,
            "tool_count": len(tools),
            "tool_names": [tool["name"] for tool in detailed_tools],

            # 新增：Resources相关字段
            "resource_count": 0,  # TODO: 实现资源数量统计
            "resource_names": [],  # TODO: 实现资源名称列表
            "resource_template_count": 0,  # TODO: 实现资源模板数量统计
            "resource_template_names": [],  # TODO: 实现资源模板名称列表

            # 新增：Prompts相关字段
            "prompt_count": 0,  # TODO: 实现提示词数量统计
            "prompt_names": [],  # TODO: 实现提示词名称列表

            # 新增：能力标识
            "capabilities": ["tools"],  # TODO: 根据实际支持的功能动态更新

            # 现有字段
            "last_heartbeat": str(last_heartbeat) if last_heartbeat else "N/A",
            "connected": name in self.sessions.get(agent_id, {})
        }

    def get_all_service_names(self, agent_id: str) -> List[str]:
        """
        获取指定 agent_id 下所有已注册服务名。
         修复：从service_states获取服务列表，而不是sessions（sessions可能为空）
        """
        return list(self.service_states.get(agent_id, {}).keys())

    def get_services_for_agent(self, agent_id: str) -> List[str]:
        """
        获取指定 agent_id 下所有已注册服务名（别名方法）
        """
        return self.get_all_service_names(agent_id)

    def get_service_info(self, agent_id: str, service_name: str) -> Optional['ServiceInfo']:
        """
        获取指定服务的基本信息

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            ServiceInfo对象或None
        """
        try:
            # 检查服务是否存在
            if service_name not in self.sessions.get(agent_id, {}):
                return None

            # 获取服务状态
            state = self.get_service_state(agent_id, service_name)

            # 获取工具数量
            tools = self.get_tools_for_service(agent_id, service_name)
            tool_count = len(tools)

            # 获取服务元数据
            metadata = self.get_service_metadata(agent_id, service_name)

            # 构造ServiceInfo对象
            from mcpstore.core.models.service import ServiceInfo, TransportType
            from datetime import datetime

            # 尝试从元数据中获取配置信息
            service_config = metadata.service_config if metadata else {}

            # 推断传输类型
            transport_type = TransportType.STREAMABLE_HTTP  # 默认
            if 'url' in service_config:
                transport_type = TransportType.STREAMABLE_HTTP
            elif 'command' in service_config:
                transport_type = TransportType.STDIO

            service_info = ServiceInfo(
                name=service_name,
                transport_type=transport_type,
                status=state,
                tool_count=tool_count,
                url=service_config.get('url', ''),
                command=service_config.get('command'),
                args=service_config.get('args'),
                working_dir=service_config.get('working_dir'),
                env=service_config.get('env'),
                keep_alive=service_config.get('keep_alive', False),
                package_name=service_config.get('package_name'),
                last_heartbeat=metadata.last_ping_time if metadata else None,
                last_state_change=metadata.state_entered_time if metadata else datetime.now(),
                state_metadata=metadata,
                config=service_config  #  [REFACTOR] 添加完整的config字段
            )

            return service_info

        except Exception as e:
            logger.debug(f"获取服务信息时出现异常: {e}")
            return None

    def update_service_health(self, agent_id: str, name: str):
        """
        更新指定 agent_id 下某服务的心跳时间。
        ⚠️ 已废弃：此方法已被ServiceLifecycleManager替代
        """
        logger.debug(f"update_service_health is deprecated for service: {name} (agent_id={agent_id})")
        pass

    def get_last_heartbeat(self, agent_id: str, name: str) -> Optional[datetime]:
        """
        获取指定 agent_id 下某服务的最后心跳时间。
        ⚠️ 已废弃：此方法已被ServiceLifecycleManager替代
        """
        logger.debug(f"get_last_heartbeat is deprecated for service: {name} (agent_id={agent_id})")
        return None

    def has_service(self, agent_id: str, name: str) -> bool:
        """
        判断指定 agent_id 下是否存在某服务。
         修复：从service_states判断服务是否存在，而不是sessions（sessions可能为空）
        """
        return name in self.service_states.get(agent_id, {})

    def get_service_config(self, agent_id: str, name: str) -> Optional[Dict[str, Any]]:
        """获取服务配置"""
        if not self.has_service(agent_id, name):
            return None

        # 从 orchestrator 的 mcp_config 获取配置
        from api.deps import app_state
        orchestrator = app_state.get("orchestrator")
        if orchestrator and orchestrator.mcp_config:
            return orchestrator.mcp_config.get_service_config(name)

        return None

    def mark_as_long_lived(self, agent_id: str, service_name: str):
        """标记服务为长连接服务"""
        service_key = f"{agent_id}:{service_name}"
        self.long_lived_connections.add(service_key)
        logger.debug(f"Marked service '{service_name}' as long-lived for agent '{agent_id}'")

    def is_long_lived_service(self, agent_id: str, service_name: str) -> bool:
        """检查服务是否为长连接服务"""
        service_key = f"{agent_id}:{service_name}"
        return service_key in self.long_lived_connections

    def get_long_lived_services(self, agent_id: str) -> List[str]:
        """获取指定Agent的所有长连接服务"""
        prefix = f"{agent_id}:"
        return [
            key[len(prefix):] for key in self.long_lived_connections
            if key.startswith(prefix)
        ]

    # === 生命周期状态管理方法 ===

    def set_service_state(self, agent_id: str, service_name: str, state: Optional[ServiceConnectionState]):
        """ [ENHANCED] 设置服务生命周期状态，自动同步共享 Client ID 的服务"""

        # 记录旧状态
        old_state = self.service_states.get(agent_id, {}).get(service_name)

        # 设置新状态（现有逻辑）
        if agent_id not in self.service_states:
            self.service_states[agent_id] = {}

        if state is None:
            # 删除状态
            if service_name in self.service_states[agent_id]:
                del self.service_states[agent_id][service_name]
                logger.debug(f"Service {service_name} (agent {agent_id}) state removed")
        else:
            # 设置状态
            self.service_states[agent_id][service_name] = state
            logger.debug(f"Service {service_name} (agent {agent_id}) state set to {state.value}")

        #  新增：自动同步共享服务状态
        if state is not None and old_state != state:
            self._ensure_state_sync_manager()
            self._state_sync_manager.sync_state_for_shared_client(agent_id, service_name, state)

    def get_service_state(self, agent_id: str, service_name: str) -> ServiceConnectionState:
        """获取服务生命周期状态"""
        return self.service_states.get(agent_id, {}).get(service_name, ServiceConnectionState.DISCONNECTED)

    def set_service_metadata(self, agent_id: str, service_name: str, metadata: Optional[ServiceStateMetadata]):
        """ [REFACTOR] 设置服务状态元数据，支持删除操作"""
        if agent_id not in self.service_metadata:
            self.service_metadata[agent_id] = {}

        if metadata is None:
            # 删除元数据
            if service_name in self.service_metadata[agent_id]:
                del self.service_metadata[agent_id][service_name]
                logger.debug(f"Service {service_name} (agent {agent_id}) metadata removed")
        else:
            # 设置元数据
            self.service_metadata[agent_id][service_name] = metadata
            logger.debug(f"Service {service_name} (agent {agent_id}) metadata updated")

    def get_service_metadata(self, agent_id: str, service_name: str) -> Optional[ServiceStateMetadata]:
        """获取服务状态元数据"""
        return self.service_metadata.get(agent_id, {}).get(service_name)

    def remove_service_lifecycle_data(self, agent_id: str, service_name: str):
        """移除服务的生命周期数据"""
        if agent_id in self.service_states:
            self.service_states[agent_id].pop(service_name, None)
        if agent_id in self.service_metadata:
            self.service_metadata[agent_id].pop(service_name, None)
        logger.debug(f"Removed lifecycle data for service {service_name} (agent {agent_id})")

    def get_all_service_states(self, agent_id: str) -> Dict[str, ServiceConnectionState]:
        """获取指定Agent的所有服务状态"""
        return self.service_states.get(agent_id, {}).copy()

    def clear_agent_lifecycle_data(self, agent_id: str):
        """清除指定Agent的所有生命周期数据"""
        self.service_states.pop(agent_id, None)
        self.service_metadata.pop(agent_id, None)
        logger.info(f"Cleared lifecycle data for agent {agent_id}")

    def should_cache_aggressively(self, agent_id: str, service_name: str) -> bool:
        """
        判断是否应该激进缓存
        长连接服务可以更激进地缓存，因为连接稳定
        """
        return self.is_long_lived_service(agent_id, service_name)

    # ===  新增：Agent-Client 映射管理 ===

    def add_agent_client_mapping(self, agent_id: str, client_id: str):
        """添加 Agent-Client 映射到缓存（委托后端）"""
        self.cache_backend.add_agent_client_mapping(agent_id, client_id)
        logger.debug(f"[REGISTRY] agent_client_mapped client_id={client_id} agent_id={agent_id}")
        logger.debug(f"[REGISTRY] agent_clients={dict(self.agent_clients)}")

    def get_all_agent_ids(self) -> List[str]:
        """ [REFACTOR] 从缓存获取所有Agent ID列表"""
        agent_ids = list(self.agent_clients.keys())
        logger.info(f"[REGISTRY] get_all_agent_ids ids={agent_ids}")
        logger.info(f"[REGISTRY] agent_clients_full={dict(self.agent_clients)}")
        return agent_ids

    def get_agent_clients_from_cache(self, agent_id: str) -> List[str]:
        """从缓存获取 Agent 的所有 Client ID"""
        return self.cache_backend.get_agent_clients_from_cache(agent_id)

    def remove_agent_client_mapping(self, agent_id: str, client_id: str):
        """从缓存移除 Agent-Client 映射（委托后端）"""
        self.cache_backend.remove_agent_client_mapping(agent_id, client_id)

    # ===  新增：Client 配置管理 ===

    def add_client_config(self, client_id: str, config: Dict[str, Any]):
        """添加 Client 配置到缓存"""
        self.cache_backend.add_client_config(client_id, config)
        logger.debug(f"Added client config for {client_id} to cache")

    def get_client_config_from_cache(self, client_id: str) -> Optional[Dict[str, Any]]:
        """从缓存获取 Client 配置"""
        return self.cache_backend.get_client_config_from_cache(client_id)

    def update_client_config(self, client_id: str, updates: Dict[str, Any]):
        """更新缓存中的 Client 配置"""
        self.cache_backend.update_client_config(client_id, updates)

    def remove_client_config(self, client_id: str):
        """从缓存移除 Client 配置"""
        self.cache_backend.remove_client_config(client_id)

    # ===  新增：Service-Client 映射管理 ===

    def add_service_client_mapping(self, agent_id: str, service_name: str, client_id: str):
        """添加 Service-Client 映射到缓存"""
        self.cache_backend.add_service_client_mapping(agent_id, service_name, client_id)
        logger.debug(f"Mapped service {service_name} to client {client_id} for agent {agent_id}")

    def get_service_client_id(self, agent_id: str, service_name: str) -> Optional[str]:
        """获取服务对应的 Client ID"""
        return self.cache_backend.get_service_client_id(agent_id, service_name)

    def remove_service_client_mapping(self, agent_id: str, service_name: str):
        """移除 Service-Client 映射"""
        self.cache_backend.remove_service_client_mapping(agent_id, service_name)


    def get_repository(self):
        """Return a Repository-style thin facade bound to this registry.
        Avoids circular import by importing locally.
        """
        try:
            from .repository import CacheRepository  # type: ignore
        except Exception as e:
            raise RuntimeError(f"CacheRepository unavailable: {e}")
        return CacheRepository(self)

    # ===  新增：Agent 服务映射管理 ===

    def add_agent_service_mapping(self, agent_id: str, local_name: str, global_name: str):
        """
        建立 Agent 服务映射关系

        Args:
            agent_id: Agent ID
            local_name: Agent 中的本地服务名
            global_name: Store 中的全局服务名（带后缀）
        """
        # 建立 agent -> global 映射
        if agent_id not in self.agent_to_global_mappings:
            self.agent_to_global_mappings[agent_id] = {}
        self.agent_to_global_mappings[agent_id][local_name] = global_name

        # 建立 global -> agent 映射
        self.global_to_agent_mappings[global_name] = (agent_id, local_name)

        logger.debug(f" [AGENT_MAPPING] Added mapping: {agent_id}:{local_name} ↔ {global_name}")

    def get_global_name_from_agent_service(self, agent_id: str, local_name: str) -> Optional[str]:
        """获取 Agent 服务对应的全局名称"""
        return self.agent_to_global_mappings.get(agent_id, {}).get(local_name)

    def get_agent_service_from_global_name(self, global_name: str) -> Optional[Tuple[str, str]]:
        """获取全局服务名对应的 Agent 服务信息"""
        return self.global_to_agent_mappings.get(global_name)

    def get_agent_services(self, agent_id: str) -> List[str]:
        """获取 Agent 的所有服务（全局名称）"""
        return list(self.agent_to_global_mappings.get(agent_id, {}).values())

    def is_agent_service(self, global_name: str) -> bool:
        """判断是否为 Agent 服务"""
        return global_name in self.global_to_agent_mappings

    def remove_agent_service_mapping(self, agent_id: str, local_name: str):
        """移除 Agent 服务映射"""
        if agent_id in self.agent_to_global_mappings:
            global_name = self.agent_to_global_mappings[agent_id].pop(local_name, None)
            if global_name:
                self.global_to_agent_mappings.pop(global_name, None)
                logger.debug(f" [AGENT_MAPPING] Removed mapping: {agent_id}:{local_name} ↔ {global_name}")

    # ===  新增：完整的服务信息获取 ===

    def get_service_summary(self, agent_id: str, service_name: str) -> Dict[str, Any]:
        """
        获取服务完整摘要信息

        Returns:
            {
                "name": "weather",
                "state": "healthy",
                "tool_count": 5,
                "tools": ["get_weather", "get_forecast"],
                "has_session": True,
                "last_heartbeat": "2024-01-01T12:00:00",
                "error_message": None,
                "config": {"url": "http://weather.com"}
            }
        """
        if not self.has_service(agent_id, service_name):
            logger.debug(f"Service not found: {service_name} for agent {agent_id}")
            return {}

        state = self.get_service_state(agent_id, service_name)
        metadata = self.get_service_metadata(agent_id, service_name)
        tools = self.get_tools_for_service(agent_id, service_name)
        session = self.get_session(agent_id, service_name)

        # 安全的时间格式化
        def safe_isoformat(dt):
            if dt is None:
                return None
            if hasattr(dt, 'isoformat'):
                return dt.isoformat()
            elif isinstance(dt, str):
                return dt
            else:
                return str(dt)

        return {
            "name": service_name,
            "state": state.value if state else "unknown",
            "tool_count": len(tools),
            "tools": tools,
            "has_session": session is not None,
            "last_heartbeat": safe_isoformat(metadata.last_ping_time if metadata else None),
            "error_message": metadata.error_message if metadata else None,
            "config": metadata.service_config if metadata else {},
            "consecutive_failures": metadata.consecutive_failures if metadata else 0,
            "state_entered_time": safe_isoformat(metadata.state_entered_time if metadata else None),
            # 修复：添加state_metadata字段，用于判断服务是否激活
            "state_metadata": metadata
        }

    def get_complete_service_info(self, agent_id: str, service_name: str) -> Dict[str, Any]:
        """获取服务的完整信息（包括 Client 信息）"""
        # 基础服务信息
        base_info = self.get_service_summary(agent_id, service_name)

        # Client 信息
        client_id = self.get_service_client_id(agent_id, service_name)
        client_config = self.get_client_config_from_cache(client_id) if client_id else {}

        # 合并信息
        complete_info = {
            **base_info,
            "client_id": client_id,
            "client_config": client_config,
            "agent_id": agent_id
        }

        return complete_info

    def get_all_services_complete_info(self, agent_id: str) -> List[Dict[str, Any]]:
        """获取 Agent 下所有服务的完整信息"""
        service_names = self.get_all_service_names(agent_id)
        return [
            self.get_complete_service_info(agent_id, service_name)
            for service_name in service_names
        ]

    # ===  新增：便捷查询方法 ===

    def get_services_by_state(self, agent_id: str, states: List['ServiceConnectionState']) -> List[str]:
        """
        按状态筛选服务

        Args:
            states: [ServiceConnectionState.HEALTHY, ServiceConnectionState.WARNING]

        Returns:
            ["service1", "service2"]
        """
        services = []
        for service_name, state in self.service_states.get(agent_id, {}).items():
            if state in states:
                services.append(service_name)
        return services

    def get_healthy_services(self, agent_id: str) -> List[str]:
        """获取健康的服务列表"""
        from mcpstore.core.models.service import ServiceConnectionState
        return self.get_services_by_state(agent_id, [
            ServiceConnectionState.HEALTHY,
            ServiceConnectionState.WARNING
        ])

    def get_failed_services(self, agent_id: str) -> List[str]:
        """获取失败的服务列表"""
        from mcpstore.core.models.service import ServiceConnectionState
        return self.get_services_by_state(agent_id, [
            ServiceConnectionState.UNREACHABLE,
            ServiceConnectionState.DISCONNECTED
        ])

    def get_services_with_tools(self, agent_id: str) -> List[str]:
        """获取有工具的服务列表"""
        services_with_tools = []
        for service_name in self.get_all_service_names(agent_id):
            tools = self.get_tools_for_service(agent_id, service_name)
            if tools:
                services_with_tools.append(service_name)
        return services_with_tools

    # ===  新增：缓存同步管理 ===

    def sync_to_client_manager(self, client_manager):
        """将缓存数据同步到 ClientManager（简化版本）"""
        try:
            # 这里可以实现具体的同步逻辑
            # 目前作为占位符，实际同步由cache_manager处理
            logger.debug("[REGISTRY] sync_to_client_manager called")

        except Exception as e:
            logger.error(f"Failed to sync registry to ClientManager: {e}")
            raise

    #  [REFACTOR] 移除重复的方法定义 - 使用上面统一的方法

    def get_service_config_from_cache(self, agent_id: str, service_name: str) -> Optional[Dict[str, Any]]:
        """从缓存获取服务配置（缓存优先架构的核心方法）"""
        metadata = self.get_service_metadata(agent_id, service_name)
        if metadata and metadata.service_config:
            return metadata.service_config

        # 如果缓存中没有配置，说明系统有问题，应该报错
        logger.error(f"Service configuration not found in cache for {service_name} in agent {agent_id}")
        logger.error("This indicates a system issue - all services should have config in cache")
        return None
