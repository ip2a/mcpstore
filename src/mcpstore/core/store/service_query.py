"""
服务查询模块
负责处理 MCPStore 的服务查询相关功能
"""

import logging
from typing import Optional, List, Dict, Any

from mcpstore.core.models.service import ServiceInfo, ServiceConnectionState, TransportType, ServiceInfoResponse

logger = logging.getLogger(__name__)


class ServiceQueryMixin:
    """服务查询 Mixin"""
    
    def check_services(self, agent_id: Optional[str] = None) -> Dict[str, str]:
        """兼容旧版API"""
        context = self.for_agent(agent_id) if agent_id else self.for_store()
        return context.check_services()

    def _infer_transport_type(self, service_config: Dict[str, Any]) -> TransportType:
        """推断服务的传输类型"""
        if not service_config:
            return TransportType.STREAMABLE_HTTP
            
        # 优先使用 transport 字段
        transport = service_config.get("transport")
        if transport:
            try:
                return TransportType(transport)
            except ValueError:
                pass
                
        # 其次根据 url 判断
        if service_config.get("url"):
            return TransportType.STREAMABLE_HTTP
            
        # 根据 command/args 判断
        cmd = (service_config.get("command") or "").lower()
        args = " ".join(service_config.get("args", [])).lower()
        
        # 检查是否为 Node.js 包
        if "npx" in cmd or "node" in cmd or "npm" in cmd:
            return TransportType.STDIO
        
        # 检查是否为 Python 包
        if "python" in cmd or "pip" in cmd or ".py" in args:
            return TransportType.STDIO
            
        return TransportType.STREAMABLE_HTTP

    async def list_services(self, id: Optional[str] = None, agent_mode: bool = False) -> List[ServiceInfo]:
        """
        纯缓存模式的服务列表获取

         新特点：
        - 完全从缓存获取数据
        - 包含完整的 Agent-Client 信息
        - 高性能，无文件IO
        """
        services_info = []

        # 1. Store模式：从缓存获取所有服务
        if not agent_mode and (not id or id == self.client_manager.global_agent_store_id):
            agent_id = self.client_manager.global_agent_store_id

            #  关键：纯缓存获取
            service_names = self.registry.get_all_service_names(agent_id)

            if not service_names:
                # 缓存为空，可能需要初始化
                logger.info("Cache is empty, you may need to add services first")
                return []

            for service_name in service_names:
                # 从缓存获取完整信息
                complete_info = self.registry.get_complete_service_info(agent_id, service_name)

                # 构建 ServiceInfo
                state = complete_info.get("state", "disconnected")
                # 确保状态是ServiceConnectionState枚举
                if isinstance(state, str):
                    try:
                        state = ServiceConnectionState(state)
                    except ValueError:
                        state = ServiceConnectionState.DISCONNECTED

                service_info = ServiceInfo(
                    url=complete_info.get("config", {}).get("url", ""),
                    name=service_name,
                    transport_type=self._infer_transport_type(complete_info.get("config", {})),
                    status=state,
                    tool_count=complete_info.get("tool_count", 0),
                    keep_alive=complete_info.get("config", {}).get("keep_alive", False),
                    working_dir=complete_info.get("config", {}).get("working_dir"),
                    env=complete_info.get("config", {}).get("env"),
                    last_heartbeat=complete_info.get("last_heartbeat"),
                    command=complete_info.get("config", {}).get("command"),
                    args=complete_info.get("config", {}).get("args"),
                    package_name=complete_info.get("config", {}).get("package_name"),
                    state_metadata=complete_info.get("state_metadata"),
                    last_state_change=complete_info.get("state_entered_time"),
                    client_id=complete_info.get("client_id"),  #  新增：Client ID 信息
                    config=complete_info.get("config", {})  #  [REFACTOR] 添加完整的config字段
                )
                services_info.append(service_info)

        # 2. Agent模式：作为“视图”，从 Store 命名空间派生服务列表
        elif agent_mode and id:
            try:
                agent_id = id
                global_agent_id = self.client_manager.global_agent_store_id

                # 通过映射获取该 Agent 的全局服务名集合
                global_service_names = self.registry.get_agent_services(agent_id)
                if not global_service_names:
                    logger.debug(f"[STORE.LIST_SERVICES] Agent {agent_id} 没有已映射的全局服务，返回空列表")
                    return services_info

                for global_name in global_service_names:
                    # 解析出本地名（显示用）并校验归属
                    parsed = self.registry.get_agent_service_from_global_name(global_name)
                    if not parsed:
                        continue
                    mapped_agent, local_name = parsed
                    if mapped_agent != agent_id:
                        continue

                    # 从全局命名空间读取该服务的完整信息
                    complete_info = self.registry.get_complete_service_info(global_agent_id, global_name)
                    if not complete_info:
                        logger.debug(f"[STORE.LIST_SERVICES] 全局缓存中未找到服务: {global_name}")
                        continue

                    # 状态枚举转换
                    state = complete_info.get("state", "disconnected")
                    if isinstance(state, str):
                        try:
                            state = ServiceConnectionState(state)
                        except ValueError:
                            state = ServiceConnectionState.DISCONNECTED

                    # 构建以本地名展示的 ServiceInfo（数据来源于全局）
                    cfg = complete_info.get("config", {})
                    service_info = ServiceInfo(
                        url=cfg.get("url", ""),
                        name=local_name or global_name,
                        transport_type=self._infer_transport_type(cfg),
                        status=state,
                        tool_count=complete_info.get("tool_count", 0),
                        keep_alive=cfg.get("keep_alive", False),
                        working_dir=cfg.get("working_dir"),
                        env=cfg.get("env"),
                        last_heartbeat=complete_info.get("last_heartbeat"),
                        command=cfg.get("command"),
                        args=cfg.get("args"),
                        package_name=cfg.get("package_name"),
                        state_metadata=complete_info.get("state_metadata"),
                        last_state_change=complete_info.get("state_entered_time"),
                        # 透明代理：client_id 使用全局命名空间的client
                        client_id=complete_info.get("client_id"),
                        config=cfg
                    )
                    services_info.append(service_info)
            except Exception as e:
                logger.error(f"[STORE.LIST_SERVICES] Agent 视图派生失败: {e}")
                return services_info

        return services_info

    async def get_service_info(self, name: str, agent_id: Optional[str] = None) -> ServiceInfoResponse:
        """
        获取服务详细信息（严格按上下文隔离）：
        - 未传 agent_id：仅在 global_agent_store 下所有 client_id 中查找服务
        - 传 agent_id：仅在该 agent_id 下所有 client_id 中查找服务

        优先级：按client_id顺序返回第一个匹配的服务
        """
        from mcpstore.core.store.client_manager import ClientManager
        client_manager: ClientManager = self.client_manager

        # 严格按上下文获取要查找的 client_ids
        if not agent_id:
            # Store上下文：只查找global_agent_store下的服务
            client_ids = self.registry.get_agent_clients_from_cache(self.client_manager.global_agent_store_id)
            context_type = "store"
        else:
            # Agent上下文：只查找指定agent下的服务
            client_ids = self.registry.get_agent_clients_from_cache(agent_id)
            context_type = f"agent({agent_id})"

        if not client_ids:
            return ServiceInfoResponse(
                success=False,
                message=f"No client_ids found for {context_type} context",
                service=None,
                tools=[],
                connected=False
            )

        # 按client_id顺序查找服务
        #  修复：服务存储在agent_id级别，而不是client_id级别
        agent_id_for_query = self.client_manager.global_agent_store_id if not agent_id else agent_id

        # === 健壮名称解析：支持在 Agent 上下文传入“本地名”或“全局名” ===
        query_names: List[str] = [name]
        from mcpstore.core.context.agent_service_mapper import AgentServiceMapper
        try:
            if agent_id:
                # 如果传入的是全局名（包含 _byagent_），尝试解析回本地名，确保在 agent 命名空间可匹配
                if AgentServiceMapper.is_any_agent_service(name):
                    parsed = self.registry.get_agent_service_from_global_name(name)
                    if parsed:
                        parsed_agent_id, local_name = parsed
                        # 仅当全局名确实属于当前 agent 时才使用解析出的本地名
                        if parsed_agent_id == agent_id and local_name:
                            query_names.append(local_name)
                else:
                    # 传入可能是本地名，同步构造对应全局名，方便后续 cross-namespace 校验
                    mapper = AgentServiceMapper(agent_id)
                    query_names.append(mapper.to_global_name(name))
        except Exception:
            pass

        service_names = self.registry.get_all_service_names(agent_id_for_query)

        # 遍历候选名称，找到第一个匹配的（在 agent 命名空间）
        match_name = next((qn for qn in query_names if qn in service_names), None)
        if match_name:
            # 推导本地名/全局名
            local_name = name
            global_name = None
            if agent_id:
                # 优先从映射表获取全局名
                global_name = self.registry.get_global_name_from_agent_service(agent_id, local_name)
                # 如果 match_name 已经是全局名，则直接使用
                if not global_name and AgentServiceMapper.is_any_agent_service(match_name):
                    global_name = match_name
                # 如果仍然没有，构造一个（不会影响存在性，仅用于读取配置）
                if not global_name:
                    mapper = AgentServiceMapper(agent_id)
                    global_name = mapper.to_global_name(local_name)
            else:
                # store 模式下，名称即全局名
                global_name = match_name

            # 确定用于读取配置/生命周期/工具的命名空间与名称
            config_key = global_name  # 单一数据源：mcp.json 使用全局名
            lifecycle_agent = self.client_manager.global_agent_store_id if agent_id else agent_id_for_query
            lifecycle_name = global_name if agent_id else match_name
            tools_agent = self.client_manager.global_agent_store_id if agent_id else agent_id_for_query
            tools_service = global_name if agent_id else match_name

            # 找到服务，需要确定它属于哪个client_id（保持 agent 视角）
            service_client_id = self.registry.get_service_client_id(agent_id_for_query, match_name)
            if service_client_id and service_client_id in client_ids:
                # 找到服务，获取详细信息
                # 从 mcp.json 读取（使用全局名）
                config = self.config.get_service_config(config_key) or {}

                # 🆕 事件驱动架构：直接从 registry 获取生命周期状态（优先全局命名空间）
                service_state = self.registry.get_service_state(lifecycle_agent, lifecycle_name)

                # 获取工具信息（优先全局命名空间）
                tool_names = self.registry.get_tools_for_service(tools_agent, tools_service)
                tools_info = []
                for tool_name in tool_names:
                    tool_info = self.registry.get_tool_info(tools_agent, tool_name)
                    if tool_info:
                        tools_info.append(tool_info)
                tool_count = len(tools_info)

                # 获取连接状态
                connected = service_state in [ServiceConnectionState.HEALTHY, ServiceConnectionState.WARNING]

                # 🆕 事件驱动架构：直接从 registry 获取元数据（不再通过 lifecycle_manager）
                service_metadata = self.registry.get_service_metadata(lifecycle_agent, lifecycle_name)

                # 构建ServiceInfo（Agent 视图下 name 使用本地名展示）
                service_info = ServiceInfo(
                    url=config.get("url", ""),
                    name=local_name if agent_id else match_name,
                    transport_type=self._infer_transport_type(config),
                    status=service_state,
                    tool_count=tool_count,
                    keep_alive=config.get("keep_alive", False),
                    working_dir=config.get("working_dir"),
                    env=config.get("env"),
                    last_heartbeat=service_metadata.last_ping_time if service_metadata else None,
                    command=config.get("command"),
                    args=config.get("args"),
                    package_name=config.get("package_name"),
                    state_metadata=service_metadata,
                    last_state_change=service_metadata.state_entered_time if service_metadata else None,
                    client_id=service_client_id,
                    config=config
                )

                return ServiceInfoResponse(
                    success=True,
                    message=f"Service found in {context_type} context (client_id: {service_client_id})",
                    service=service_info,
                    tools=tools_info,
                    connected=connected
                )

        # 未找到服务
        return ServiceInfoResponse(
            success=False,
            message=f"Service '{name}' not found in {context_type} context (searched {len(client_ids)} clients)",
            service=None,
            tools=[],
            connected=False
        )

    async def get_health_status(self, id: Optional[str] = None, agent_mode: bool = False) -> Dict[str, Any]:
        # NOTE:
        # 统一采用“按 Agent 命名空间存储服务”的约定：
        # - store 视角：使用 global_agent_store 作为命名空间
        # - agent 视角：使用指定 agent_id 作为命名空间
        # client_id 仅用于标注归属与过滤，不作为生命周期与配置的读写命名空间
        """
        获取服务健康状态：
        - store未传id 或 id==global_agent_store：聚合 global_agent_store 下所有 client_id 的服务健康状态
        - store传普通 client_id：只查该 client_id 下的服务健康状态
        - agent级别：聚合 agent_id 下所有 client_id 的服务健康状态；如果 id 不是 agent_id，尝试作为 client_id 查
        """
        from mcpstore.core.store.client_manager import ClientManager
        client_manager: ClientManager = self.client_manager
        services = []
        # 1. store未传id 或 id==global_agent_store，聚合 global_agent_store 下所有 client_id 的服务健康状态
        if not agent_mode and (not id or id == self.client_manager.global_agent_store_id):
            agent_ns = self.client_manager.global_agent_store_id
            # 在 Agent 命名空间读取所有服务，再标注其归属 client_id
            service_names = self.registry.get_all_service_names(agent_ns)
            for name in service_names:
                config = self.config.get_service_config(name) or {}
                # 生命周期与元数据：按 Agent 命名空间读取
                service_state = self.registry.get_service_state(agent_ns, name)
                state_metadata = self.registry.get_service_metadata(agent_ns, name)
                # 标注该服务当前映射到哪个 client_id（若存在）
                client_id = self.registry.get_service_client_id(agent_ns, name)

                service_status = {
                    "name": name,
                    "url": config.get("url", ""),
                    "transport_type": config.get("transport", ""),
                    "status": service_state.value if hasattr(service_state, "value") else str(service_state),
                    "command": config.get("command"),
                    "args": config.get("args"),
                    "package_name": config.get("package_name"),
                    "client_id": client_id,
                    # 生命周期元数据
                    "response_time": getattr(state_metadata, "response_time", None) if state_metadata else None,
                    "consecutive_failures": getattr(state_metadata, "consecutive_failures", 0) if state_metadata else 0,
                    "last_state_change": (state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None)
                }
                services.append(service_status)
            return {
                "orchestrator_status": "running",
                "active_services": len(services),
                "services": services
            }
        # 2. store传普通 client_id，只查该 client_id 下的服务健康状态
        if not agent_mode and id:
            if id == self.client_manager.global_agent_store_id:
                return {
                    "orchestrator_status": "running",
                    "active_services": 0,
                    "services": []
                }
            # 仅返回当前 client_id 映射到的服务（仍按 Agent 命名空间读状态）
            agent_ns = self.client_manager.global_agent_store_id
            all_names = self.registry.get_all_service_names(agent_ns)
            for name in all_names:
                mapped = self.registry.get_service_client_id(agent_ns, name)
                if mapped != id:
                    continue
                config = self.config.get_service_config(name) or {}
                service_state = self.registry.get_service_state(agent_ns, name)
                state_metadata = self.registry.get_service_metadata(agent_ns, name)
                service_status = {
                    "name": name,
                    "url": config.get("url", ""),
                    "transport_type": config.get("transport", ""),
                    "status": service_state.value if hasattr(service_state, "value") else str(service_state),
                    "command": config.get("command"),
                    "args": config.get("args"),
                    "package_name": config.get("package_name"),
                    "client_id": mapped,
                    "response_time": getattr(state_metadata, "response_time", None) if state_metadata else None,
                    "consecutive_failures": getattr(state_metadata, "consecutive_failures", 0) if state_metadata else 0,
                    "last_state_change": (state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None)
                }
                services.append(service_status)
            return {
                "orchestrator_status": "running",
                "active_services": len(services),
                "services": services
            }
        # 3. agent级别，聚合 agent_id 下所有 client_id 的服务健康状态；如果 id 不是 agent_id，尝试作为 client_id 查
        if agent_mode and id:
            client_ids = self.registry.get_agent_clients_from_cache(id)
            if client_ids:
                agent_ns = id
                names = self.registry.get_all_service_names(agent_ns)
                for name in names:
                    config = self.config.get_service_config(name) or {}
                    service_state = self.registry.get_service_state(agent_ns, name)
                    state_metadata = self.registry.get_service_metadata(agent_ns, name)
                    mapped_client = self.registry.get_service_client_id(agent_ns, name)
                    if mapped_client not in (client_ids or []):
                        continue
                    service_status = {
                        "name": name,
                        "url": config.get("url", ""),
                        "transport_type": config.get("transport", ""),
                        "status": service_state.value if hasattr(service_state, "value") else str(service_state),
                        "command": config.get("command"),
                        "args": config.get("args"),
                        "package_name": config.get("package_name"),
                        "client_id": mapped_client,
                        "response_time": getattr(state_metadata, "response_time", None) if state_metadata else None,
                        "consecutive_failures": getattr(state_metadata, "consecutive_failures", 0) if state_metadata else 0,
                        "last_state_change": (state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None)
                    }
                    services.append(service_status)
                return {
                    "orchestrator_status": "running",
                    "active_services": len(services),
                    "services": services
                }
            else:
                # id 不是 agent_id，则视为 client_id：过滤 agent 命名空间下映射到该 client 的服务
                agent_ns = self.client_manager.global_agent_store_id
                names = self.registry.get_all_service_names(agent_ns)
                for name in names:
                    mapped_client = self.registry.get_service_client_id(agent_ns, name)
                    if mapped_client != id:
                        continue
                    config = self.config.get_service_config(name) or {}
                    service_state = self.registry.get_service_state(agent_ns, name)
                    state_metadata = self.registry.get_service_metadata(agent_ns, name)
                    service_status = {
                        "name": name,
                        "url": config.get("url", ""),
                        "transport_type": config.get("transport", ""),
                        "status": service_state.value if hasattr(service_state, "value") else str(service_state),
                        "command": config.get("command"),
                        "args": config.get("args"),
                        "package_name": config.get("package_name"),
                        "client_id": mapped_client,
                        "response_time": getattr(state_metadata, "response_time", None) if state_metadata else None,
                        "consecutive_failures": getattr(state_metadata, "consecutive_failures", 0) if state_metadata else 0,
                        "last_state_change": (state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None)
                    }
                    services.append(service_status)
                return {
                    "orchestrator_status": "running",
                    "active_services": len(services),
                    "services": services
                }
        return {
            "orchestrator_status": "running",
            "active_services": 0,
            "services": []
        }
