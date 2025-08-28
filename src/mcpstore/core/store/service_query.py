"""
服务查询模块
负责处理 MCPStore 的服务查询相关功能
"""

from typing import Optional, List, Dict, Any
import logging

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

        🔧 新特点：
        - 完全从缓存获取数据
        - 包含完整的 Agent-Client 信息
        - 高性能，无文件IO
        """
        services_info = []

        # 1. Store模式：从缓存获取所有服务
        if not agent_mode and (not id or id == self.client_manager.global_agent_store_id):
            agent_id = self.client_manager.global_agent_store_id

            # 🔧 关键：纯缓存获取
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
                    client_id=complete_info.get("client_id"),  # 🔧 新增：Client ID 信息
                    config=complete_info.get("config", {})  # 🔧 [REFACTOR] 添加完整的config字段
                )
                services_info.append(service_info)

        # 2. Agent模式：从缓存获取 Agent 的服务
        elif agent_mode and id:
            service_names = self.registry.get_all_service_names(id)

            for service_name in service_names:
                complete_info = self.registry.get_complete_service_info(id, service_name)

                # Agent模式可能需要名称映射
                display_name = service_name
                if hasattr(self, '_service_mapper') and self._service_mapper:
                    display_name = self._service_mapper.to_local_name(service_name)

                # 确保状态是ServiceConnectionState枚举
                state = complete_info.get("state", "disconnected")
                if isinstance(state, str):
                    try:
                        state = ServiceConnectionState(state)
                    except ValueError:
                        state = ServiceConnectionState.DISCONNECTED

                service_info = ServiceInfo(
                    url=complete_info.get("config", {}).get("url", ""),
                    name=display_name,  # 显示本地名称
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
                    client_id=complete_info.get("client_id"),
                    config=complete_info.get("config", {})  # 🔧 [REFACTOR] 添加完整的config字段
                )
                services_info.append(service_info)

        return services_info

    async def get_service_info(self, name: str, agent_id: Optional[str] = None) -> ServiceInfoResponse:
        """
        获取服务详细信息（严格按上下文隔离）：
        - 未传 agent_id：仅在 global_agent_store 下所有 client_id 中查找服务
        - 传 agent_id：仅在该 agent_id 下所有 client_id 中查找服务

        优先级：按client_id顺序返回第一个匹配的服务
        """
        from mcpstore.core.client_manager import ClientManager
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
        # 🔧 修复：服务存储在agent_id级别，而不是client_id级别
        agent_id_for_query = self.client_manager.global_agent_store_id if not agent_id else agent_id
        service_names = self.registry.get_all_service_names(agent_id_for_query)
        
        if name in service_names:
            # 找到服务，需要确定它属于哪个client_id
            service_client_id = self.registry.get_service_client_id(agent_id_for_query, name)
            if service_client_id and service_client_id in client_ids:
                # 找到服务，获取详细信息
                config = self.config.get_service_config(name) or {}

                # 获取生命周期状态
                service_state = self.orchestrator.lifecycle_manager.get_service_state(agent_id_for_query, name)

                # 获取工具信息
                # 🔧 修复：使用正确的方法获取特定服务的工具信息
                tool_names = self.registry.get_tools_for_service(agent_id_for_query, name)
                tools_info = []
                for tool_name in tool_names:
                    tool_info = self.registry.get_tool_info(agent_id_for_query, tool_name)
                    if tool_info:
                        tools_info.append(tool_info)
                tool_count = len(tools_info)

                # 获取连接状态
                connected = service_state in [ServiceConnectionState.HEALTHY, ServiceConnectionState.WARNING]

                # 🔧 修复：获取真实的生命周期数据
                service_metadata = self.orchestrator.lifecycle_manager.get_service_metadata(agent_id_for_query, name)
                
                # 构建ServiceInfo
                service_info = ServiceInfo(
                    url=config.get("url", ""),
                    name=name,
                    transport_type=self._infer_transport_type(config),
                    status=service_state,
                    tool_count=tool_count,
                    keep_alive=config.get("keep_alive", False),
                    working_dir=config.get("working_dir"),
                    env=config.get("env"),
                    last_heartbeat=service_metadata.last_ping_time if service_metadata else None,  # 🔧 真实数据
                    command=config.get("command"),
                    args=config.get("args"),
                    package_name=config.get("package_name"),
                    state_metadata=service_metadata,  # 🔧 真实数据
                    last_state_change=service_metadata.state_entered_time if service_metadata else None,  # 🔧 真实数据
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
        # TODO:该方法带完善 这个方法有一定的混乱 要分离面向用户的直观方法名 和面向业务的独立函数功能
        """
        获取服务健康状态：
        - store未传id 或 id==global_agent_store：聚合 global_agent_store 下所有 client_id 的服务健康状态
        - store传普通 client_id：只查该 client_id 下的服务健康状态
        - agent级别：聚合 agent_id 下所有 client_id 的服务健康状态；如果 id 不是 agent_id，尝试作为 client_id 查
        """
        from mcpstore.core.client_manager import ClientManager
        client_manager: ClientManager = self.client_manager
        services = []
        # 1. store未传id 或 id==global_agent_store，聚合 global_agent_store 下所有 client_id 的服务健康状态
        if not agent_mode and (not id or id == self.client_manager.global_agent_store_id):
            client_ids = self.registry.get_agent_clients_from_cache(self.client_manager.global_agent_store_id)
            for client_id in client_ids:
                service_names = self.registry.get_all_service_names(client_id)
                for name in service_names:
                    config = self.config.get_service_config(name) or {}

                    # 获取生命周期状态
                    service_state = self.orchestrator.lifecycle_manager.get_service_state(client_id, name)
                    state_metadata = self.orchestrator.lifecycle_manager.get_service_metadata(client_id, name)

                    service_status = {
                        "name": name,
                        "url": config.get("url", ""),
                        "transport_type": config.get("transport", ""),
                        "status": service_state.value,  # 使用新的7状态枚举
                        "command": config.get("command"),
                        "args": config.get("args"),
                        "package_name": config.get("package_name"),
                        # 新增生命周期相关信息
                        "response_time": state_metadata.response_time if state_metadata else None,
                        "consecutive_failures": state_metadata.consecutive_failures if state_metadata else 0,
                        "last_state_change": state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None
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
            service_names = self.registry.get_all_service_names(id)
            for name in service_names:
                config = self.config.get_service_config(name) or {}

                # 获取生命周期状态
                service_state = self.orchestrator.lifecycle_manager.get_service_state(id, name)
                state_metadata = self.orchestrator.lifecycle_manager.get_service_metadata(id, name)

                service_status = {
                    "name": name,
                    "url": config.get("url", ""),
                    "transport_type": config.get("transport", ""),
                    "status": service_state.value,  # 使用新的7状态枚举
                    "command": config.get("command"),
                    "args": config.get("args"),
                    "package_name": config.get("package_name"),
                    # 新增生命周期相关信息
                    "response_time": state_metadata.response_time if state_metadata else None,
                    "consecutive_failures": state_metadata.consecutive_failures if state_metadata else 0,
                    "last_state_change": state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None
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
                for client_id in client_ids:
                    service_names = self.registry.get_all_service_names(client_id)
                    for name in service_names:
                        config = self.config.get_service_config(name) or {}

                        # 获取生命周期状态
                        service_state = self.orchestrator.lifecycle_manager.get_service_state(client_id, name)
                        state_metadata = self.orchestrator.lifecycle_manager.get_service_metadata(client_id, name)

                        service_status = {
                            "name": name,
                            "url": config.get("url", ""),
                            "transport_type": config.get("transport", ""),
                            "status": service_state.value,  # 使用新的7状态枚举
                            "command": config.get("command"),
                            "args": config.get("args"),
                            "package_name": config.get("package_name"),
                            # 新增生命周期相关信息
                            "response_time": state_metadata.response_time if state_metadata else None,
                            "consecutive_failures": state_metadata.consecutive_failures if state_metadata else 0,
                            "last_state_change": state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None
                        }
                        services.append(service_status)
                return {
                    "orchestrator_status": "running",
                    "active_services": len(services),
                    "services": services
                }
            else:
                service_names = self.registry.get_all_service_names(id)
                for name in service_names:
                    config = self.config.get_service_config(name) or {}

                    # 获取生命周期状态
                    service_state = self.orchestrator.lifecycle_manager.get_service_state(id, name)
                    state_metadata = self.orchestrator.lifecycle_manager.get_service_metadata(id, name)

                    service_status = {
                        "name": name,
                        "url": config.get("url", ""),
                        "transport_type": config.get("transport", ""),
                        "status": service_state.value,  # 使用新的7状态枚举
                        "command": config.get("command"),
                        "args": config.get("args"),
                        "package_name": config.get("package_name"),
                        # 新增生命周期相关信息
                        "response_time": state_metadata.response_time if state_metadata else None,
                        "consecutive_failures": state_metadata.consecutive_failures if state_metadata else 0,
                        "last_state_change": state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None
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
