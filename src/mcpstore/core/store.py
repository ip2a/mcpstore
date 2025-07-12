from mcpstore.core.orchestrator import MCPOrchestrator
from mcpstore.core.registry import ServiceRegistry
from mcpstore.config.json_config import MCPConfig
from mcpstore.core.client_manager import ClientManager
from mcpstore.core.session_manager import SessionManager
from mcpstore.core.unified_config import UnifiedConfigManager
from mcpstore.core.models.service import (
    RegisterRequestUnion, JsonUpdateRequest,
    ServiceInfo, ServicesResponse, TransportType, ServiceInfoResponse
)
from mcpstore.core.models.client import ClientRegistrationRequest
from mcpstore.core.models.tool import (
    ToolInfo, ToolsResponse, ToolExecutionRequest
)
from mcpstore.core.models.common import (
    RegistrationResponse, ConfigResponse, ExecutionResponse
)
import logging
from typing import Optional, List, Dict, Any, Union
from .context import MCPStoreContext

logger = logging.getLogger(__name__)

class MCPStore:
    """
    MCPStore - 智能体工具服务商店
    提供上下文切换的入口和通用操作
    """
    def __init__(self, orchestrator: MCPOrchestrator, config: MCPConfig):
        self.orchestrator = orchestrator
        self.config = config
        self.registry = orchestrator.registry
        self.client_manager = orchestrator.client_manager
        self.session_manager = orchestrator.session_manager
        self.logger = logging.getLogger(__name__)

        # 统一配置管理器
        self._unified_config = UnifiedConfigManager(
            mcp_config_path=config.json_path,
            client_services_path=self.client_manager.services_path
        )

        self._context_cache: Dict[str, MCPStoreContext] = {}
        self._store_context = self._create_store_context()

    def _create_store_context(self) -> MCPStoreContext:
        """创建商店级别的上下文"""
        return MCPStoreContext(self)

    @staticmethod
    def setup_store(mcp_config_file: str = None, debug: bool = False):
        """
        初始化MCPStore实例

        Args:
            mcp_config_file: 自定义mcp.json配置文件路径，如果不指定则使用默认路径
            debug: 是否启用调试日志，默认为False（不显示调试信息）

        Returns:
            MCPStore实例
        """
        # 配置日志
        from mcpstore.config.config import LoggingConfig
        LoggingConfig.setup_logging(debug=debug)

        config = MCPConfig(json_path=mcp_config_file)
        registry = ServiceRegistry()
        orchestrator = MCPOrchestrator(config.load_config(), registry)
        return MCPStore(orchestrator, config)
  
    def _create_agent_context(self, agent_id: str) -> MCPStoreContext:
        """创建agent级别的上下文"""
        return MCPStoreContext(self, agent_id)

    def for_store(self) -> MCPStoreContext:
        """获取商店级别的上下文"""
        # main_client 作为 store agent_id
        return self._store_context

    def for_agent(self, agent_id: str) -> MCPStoreContext:
        """获取agent级别的上下文（带缓存）"""
        if agent_id not in self._context_cache:
            self._context_cache[agent_id] = self._create_agent_context(agent_id)
        return self._context_cache[agent_id]

    def get_unified_config(self) -> UnifiedConfigManager:
        """获取统一配置管理器

        Returns:
            UnifiedConfigManager: 统一配置管理器实例
        """
        return self._unified_config

    async def register_service(self, payload: RegisterRequestUnion, agent_id: Optional[str] = None) -> Dict[str, str]:
        """重构：注册服务，支持批量 service_names 注册"""
        service_names = getattr(payload, 'service_names', None)
        if not service_names:
            raise ValueError("payload 必须包含 service_names 字段")
        results = {}
        agent_key = agent_id or self.client_manager.main_client_id
        for name in service_names:
            success, msg = await self.orchestrator.connect_service(name)
            if not success:
                results[name] = f"连接失败: {msg}"
                continue
            session = self.registry.get_session(agent_key, name)
            if not session:
                results[name] = "未能获取 session"
                continue
            tools = []
            try:
                tools = await session.list_tools() if hasattr(session, 'list_tools') else []
            except Exception as e:
                results[name] = f"获取工具失败: {e}"
                continue
            added_tools = self.registry.add_service(agent_key, name, session, [(tool['name'], tool) for tool in tools])
            results[name] = f"注册成功，工具数: {len(added_tools)}"
        return results

    async def register_json_service(self, client_id: Optional[str] = None, service_names: Optional[List[str]] = None) -> RegistrationResponse:
        """
        TODO：名字不直观，作用模糊，梳理这个函数的上下使用 准备重构
        批量注册服务，支持多种场景：
        1. Store 全量注册：client_id == main_client_id，不指定 service_names
        2. Agent 指定服务注册：提供 client_id 和 service_names
        3. 临时注册：不提供 client_id，但提供 service_names
        4. 默认全量注册：既不提供 client_id 也不提供 service_names
        
        Args:
            client_id: 客户端ID，可选
            service_names: 服务名称列表，可选
            
        Returns:
            RegistrationResponse: 注册结果
        """
        try:
            # 重新加载配置以确保使用最新配置

            all_services = self.config.load_config().get("mcpServers", {})
            
            # 情况1: Store 全量注册
            if client_id and client_id == self.client_manager.main_client_id and not service_names:
                logger.info(f"STORE模式-全量注册，client_id: {client_id}")
                agent_id = self.client_manager.main_client_id
                registered_client_ids = []
                registered_services = []
                
                for name in all_services.keys():
                    try:
                        # 🔧 修复：使用同名服务处理逻辑
                        success = self.client_manager.replace_service_in_agent(
                            agent_id=agent_id,
                            service_name=name,
                            new_service_config=all_services[name]
                        )
                        if not success:
                            logger.error(f"替换服务 {name} 失败")
                            continue

                        # 获取刚创建/更新的client_id用于Registry注册
                        client_ids = self.client_manager.get_agent_clients(agent_id)
                        for client_id_check in client_ids:
                            client_config = self.client_manager.get_client_config(client_id_check)
                            if client_config and name in client_config.get("mcpServers", {}):
                                await self.orchestrator.register_json_services(client_config, client_id=client_id_check)
                                registered_client_ids.append(client_id_check)
                                registered_services.append(name)
                                logger.info(f"成功注册服务: {name}")
                                break
                    except Exception as e:
                        logger.error(f"注册服务 {name} 失败: {e}")
                        continue
                        
                return RegistrationResponse(
                    success=True,
                    client_id=agent_id,
                    service_names=registered_services,
                    config={"client_ids": registered_client_ids, "services": registered_services}
                )
                
            # 情况2: 临时注册（不提供client_id但提供service_names）
            elif not client_id and service_names:
                logger.info(f"临时注册模式，services: {service_names}")
                config = self.orchestrator.create_client_config_from_names(service_names)
                import time; agent_id = f"agent_{int(time.time() * 1000)}"
                results = await self.orchestrator.register_json_services(config)
                return RegistrationResponse(
                    success=True,
                    client_id=agent_id,
                    service_names=list(results.get("services", {}).keys()),
                    config=config
                )
                
            # 情况3: 默认全量注册
            elif not client_id and not service_names:
                logger.info("默认全量注册")
                # 直接执行全量注册逻辑，避免递归调用
                agent_id = self.client_manager.main_client_id
                registered_client_ids = []
                registered_services = []

                for name in all_services.keys():
                    try:
                        # 🔧 修复：使用同名服务处理逻辑
                        success = self.client_manager.replace_service_in_agent(
                            agent_id=agent_id,
                            service_name=name,
                            new_service_config=all_services[name]
                        )
                        if not success:
                            logger.error(f"替换服务 {name} 失败")
                            continue

                        # 获取刚创建/更新的client_id用于Registry注册
                        client_ids = self.client_manager.get_agent_clients(agent_id)
                        for client_id_check in client_ids:
                            client_config = self.client_manager.get_client_config(client_id_check)
                            if client_config and name in client_config.get("mcpServers", {}):
                                await self.orchestrator.register_json_services(client_config, client_id=client_id_check)
                                registered_client_ids.append(client_id_check)
                                registered_services.append(name)
                                logger.info(f"成功注册服务: {name}")
                                break
                    except Exception as e:
                        logger.error(f"注册服务 {name} 失败: {e}")
                        continue

                return RegistrationResponse(
                    success=True,
                    client_id=agent_id,
                    service_names=registered_services,
                    config={"client_ids": registered_client_ids, "services": registered_services}
                )
                
            # 情况4: Agent 指定服务注册
            else:
                logger.info(f"AGENT模式-指定服务注册，client_id: {client_id}, services: {service_names}")
                agent_id = client_id
                registered_client_ids = []
                registered_services = []
                
                for name in service_names or []:
                    try:
                        if name not in all_services:
                            logger.warning(f"服务 {name} 未在全局配置中找到，跳过")
                            continue

                        # 🔧 修复：使用同名服务处理逻辑
                        success = self.client_manager.replace_service_in_agent(
                            agent_id=agent_id,
                            service_name=name,
                            new_service_config=all_services[name]
                        )
                        if not success:
                            logger.error(f"替换服务 {name} 失败")
                            continue

                        # 获取刚创建/更新的client_id用于Registry注册
                        client_ids = self.client_manager.get_agent_clients(agent_id)
                        for client_id_check in client_ids:
                            client_config = self.client_manager.get_client_config(client_id_check)
                            if client_config and name in client_config.get("mcpServers", {}):
                                await self.orchestrator.register_json_services(client_config, client_id=client_id_check)
                                registered_client_ids.append(client_id_check)
                                registered_services.append(name)
                                logger.info(f"成功注册服务: {name}")
                                break
                    except Exception as e:
                        logger.error(f"注册服务 {name} 失败: {e}")
                        continue
                        
                return RegistrationResponse(
                    success=True,
                    client_id=agent_id,
                    service_names=registered_services,
                    config={"client_ids": registered_client_ids, "services": registered_services}
                )
                
        except Exception as e:
            logger.error(f"服务注册失败: {e}")
            return RegistrationResponse(
                success=False,
                message=str(e),
                client_id=client_id or self.client_manager.main_client_id,
                service_names=[],
                config={}
            )

    async def update_json_service(self, payload: JsonUpdateRequest) -> RegistrationResponse:
        """更新服务配置，等价于 PUT /register/json"""
        results = await self.orchestrator.register_json_services(
            config=payload.config,
            client_id=payload.client_id
        )
        return RegistrationResponse(
            success=True,
            client_id=results.get("client_id", payload.client_id or "main_client"),
            service_names=list(results.get("services", {}).keys()),
            config=payload.config
        )

    def get_json_config(self, client_id: Optional[str] = None) -> ConfigResponse:
        """查询服务配置，等价于 GET /register/json"""
        if not client_id or client_id == self.client_manager.main_client_id:
            config = self.config.load_config()
            return ConfigResponse(
                success=True,
                client_id=self.client_manager.main_client_id,
                config=config
            )
        else:
            config = self.client_manager.get_client_config(client_id)
            if not config:
                raise ValueError(f"Client configuration not found: {client_id}")
            return ConfigResponse(
                success=True,
                client_id=client_id,
                config=config
            )

    async def process_tool_request(self, request: ToolExecutionRequest) -> ExecutionResponse:
        """
        处理工具执行请求（FastMCP 标准）

        Args:
            request: 工具执行请求

        Returns:
            ExecutionResponse: 工具执行响应
        """
        try:
            # 验证请求参数
            if not request.tool_name:
                raise ValueError("Tool name cannot be empty")
            if not request.service_name:
                raise ValueError("Service name cannot be empty")

            logger.debug(f"Processing tool request: {request.service_name}::{request.tool_name}")

            # 执行工具（使用 FastMCP 标准）
            result = await self.orchestrator.execute_tool_fastmcp(
                service_name=request.service_name,
                tool_name=request.tool_name,
                arguments=request.args,
                agent_id=request.agent_id,
                timeout=request.timeout,
                progress_handler=request.progress_handler,
                raise_on_error=request.raise_on_error
            )

            return ExecutionResponse(
                success=True,
                result=result
            )
        except Exception as e:
            logger.error(f"Tool execution failed: {e}")
            return ExecutionResponse(
                success=False,
                error=str(e)
            )

    def register_clients(self, client_configs: Dict[str, Any]) -> RegistrationResponse:
        """注册客户端，等价于 /register_clients"""
        # 这里只是示例，具体实现需根据 client_manager 逻辑完善
        for client_id, config in client_configs.items():
            self.client_manager.save_client_config(client_id, config)
        return RegistrationResponse(
            success=True,
            message="Clients registered successfully",
            client_id="",  # 多客户端注册时不适用
            service_names=[],  # 多客户端注册时不适用
            config={"client_ids": list(client_configs.keys())}
        )

    async def get_health_status(self, id: Optional[str] = None, agent_mode: bool = False) -> Dict[str, Any]:
        """
        获取服务健康状态：
        - store未传id 或 id==main_client：聚合 main_client 下所有 client_id 的服务健康状态
        - store传普通 client_id：只查该 client_id 下的服务健康状态
        - agent级别：聚合 agent_id 下所有 client_id 的服务健康状态；如果 id 不是 agent_id，尝试作为 client_id 查
        """
        from mcpstore.core.client_manager import ClientManager
        client_manager: ClientManager = self.client_manager
        services = []
        # 1. store未传id 或 id==main_client，聚合 main_client 下所有 client_id 的服务健康状态
        if not agent_mode and (not id or id == self.client_manager.main_client_id):
            client_ids = client_manager.get_agent_clients(self.client_manager.main_client_id)
            for client_id in client_ids:
                service_names = self.registry.get_all_service_names(client_id)
                for name in service_names:
                    config = self.config.get_service_config(name) or {}
                    is_healthy = await self.orchestrator.is_service_healthy(name, client_id)
                    service_status = {
                        "name": name,
                        "url": config.get("url", ""),
                        "transport_type": config.get("transport", ""),
                        "status": "healthy" if is_healthy else "unhealthy",
                        "command": config.get("command"),
                        "args": config.get("args"),
                        "package_name": config.get("package_name")
                    }
                    services.append(service_status)
            return {
                "orchestrator_status": "running",
                "active_services": len(services),
                "services": services
            }
        # 2. store传普通 client_id，只查该 client_id 下的服务健康状态
        if not agent_mode and id:
            if id == self.client_manager.main_client_id:
                return {
                    "orchestrator_status": "running",
                    "active_services": 0,
                    "services": []
                }
            service_names = self.registry.get_all_service_names(id)
            for name in service_names:
                config = self.config.get_service_config(name) or {}
                is_healthy = await self.orchestrator.is_service_healthy(name, id)
                service_status = {
                    "name": name,
                    "url": config.get("url", ""),
                    "transport_type": config.get("transport", ""),
                    "status": "healthy" if is_healthy else "unhealthy",
                    "command": config.get("command"),
                    "args": config.get("args"),
                    "package_name": config.get("package_name")
                }
                services.append(service_status)
            return {
                "orchestrator_status": "running",
                "active_services": len(services),
                "services": services
            }
        # 3. agent级别，聚合 agent_id 下所有 client_id 的服务健康状态；如果 id 不是 agent_id，尝试作为 client_id 查
        if agent_mode and id:
            client_ids = client_manager.get_agent_clients(id)
            if client_ids:
                for client_id in client_ids:
                    service_names = self.registry.get_all_service_names(client_id)
                    for name in service_names:
                        config = self.config.get_service_config(name) or {}
                        is_healthy = await self.orchestrator.is_service_healthy(name, client_id)
                        service_status = {
                            "name": name,
                            "url": config.get("url", ""),
                            "transport_type": config.get("transport", ""),
                            "status": "healthy" if is_healthy else "unhealthy",
                            "command": config.get("command"),
                            "args": config.get("args"),
                            "package_name": config.get("package_name")
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
                    is_healthy = await self.orchestrator.is_service_healthy(name, id)
                    service_status = {
                        "name": name,
                        "url": config.get("url", ""),
                        "transport_type": config.get("transport", ""),
                        "status": "healthy" if is_healthy else "unhealthy",
                        "command": config.get("command"),
                        "args": config.get("args"),
                        "package_name": config.get("package_name")
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

    async def get_service_info(self, name: str, agent_id: Optional[str] = None) -> ServiceInfoResponse:
        """
        获取服务详细信息（严格按上下文隔离）：
        - 未传 agent_id：仅在 main_client 下所有 client_id 中查找服务
        - 传 agent_id：仅在该 agent_id 下所有 client_id 中查找服务

        优先级：按client_id顺序返回第一个匹配的服务
        """
        from mcpstore.core.client_manager import ClientManager
        client_manager: ClientManager = self.client_manager

        # 严格按上下文获取要查找的 client_ids
        if not agent_id:
            # Store上下文：只查找main_client下的服务
            client_ids = client_manager.get_agent_clients(self.client_manager.main_client_id)
            context_type = "store"
        else:
            # Agent上下文：只查找指定agent下的服务
            client_ids = client_manager.get_agent_clients(agent_id)
            context_type = f"agent({agent_id})"

        if not client_ids:
            self.logger.debug(f"No clients found for {context_type} context")
            return ServiceInfoResponse(service=None, tools=[], connected=False)

        self.logger.debug(f"Searching for service '{name}' in {context_type} context, clients: {client_ids}")

        # 按优先级在相关的 client 中查找服务（返回第一个匹配的）
        for client_id in client_ids:
            if self.registry.has_service(client_id, name):
                self.logger.debug(f"Found service '{name}' in client '{client_id}' for {context_type}")

                # 获取服务配置
                config = self.config.get_service_config(name) or {}
                service_tools = self.registry.get_tools_for_service(client_id, name)

                # 获取工具详细信息
                detailed_tools = []
                for tool_name in service_tools:
                    tool_info = self.registry._get_detailed_tool_info(client_id, tool_name)
                    if tool_info:
                        detailed_tools.append(tool_info)

                # 获取服务健康状态
                is_healthy = await self.orchestrator.is_service_healthy(name, client_id)

                # 构建服务信息（包含client_id用于调试）
                service_info = ServiceInfo(
                    url=config.get("url", ""),
                    name=name,
                    transport_type=self._infer_transport_type(config),
                    status="healthy" if is_healthy else "unhealthy",
                    tool_count=len(service_tools),
                    keep_alive=config.get("keep_alive", False),
                    working_dir=config.get("working_dir"),
                    env=config.get("env"),
                    last_heartbeat=self.registry.get_last_heartbeat(client_id, name),
                    command=config.get("command"),
                    args=config.get("args"),
                    package_name=config.get("package_name")
                )

                return ServiceInfoResponse(
                    service=service_info,
                    tools=detailed_tools,
                    connected=True
                )

        self.logger.debug(f"Service '{name}' not found in any client for {context_type}")
        return ServiceInfoResponse(
            service=None,
            tools=[],
            connected=False
        )

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
        
        if "python" in cmd or ".py" in args:
            return TransportType.STDIO_PYTHON
        if "node" in cmd or ".js" in args:
            return TransportType.STDIO_NODE
        if "uvx" in cmd:
            return TransportType.STDIO  # 使用通用的STDIO类型
        if "npx" in cmd:
            return TransportType.STDIO  # 使用通用的STDIO类型
            
        return TransportType.STREAMABLE_HTTP

    async def list_services(self, id: Optional[str] = None, agent_mode: bool = False) -> List[ServiceInfo]:
        """
        获取服务列表：
        - store未传id 或 id==main_client：聚合 main_client 下所有 client_id 的服务
        - store传普通 client_id：只查该 client_id 下的服务
        - agent级别：聚合 agent_id 下所有 client_id 的服务；如果 id 不是 agent_id，尝试作为 client_id 查
        """
        from mcpstore.core.client_manager import ClientManager
        client_manager: ClientManager = self.client_manager
        services_info = []
        # 1. store未传id 或 id==main_client，聚合 main_client 下所有 client_id 的服务
        if not agent_mode and (not id or id == self.client_manager.main_client_id):
            client_ids = client_manager.get_agent_clients(self.client_manager.main_client_id)
            for client_id in client_ids:
                service_names = self.registry.get_all_service_names(client_id)
                for name in service_names:
                    details = self.registry.get_service_details(client_id, name)
                    config = self.config.get_service_config(name) or {}
                    is_healthy = await self.orchestrator.is_service_healthy(name, client_id)
                    service_info = ServiceInfo(
                        url=config.get("url", ""),
                        name=name,
                        transport_type=self._infer_transport_type(config),
                        status="healthy" if is_healthy else "unhealthy",
                        tool_count=details.get("tool_count", 0),
                        keep_alive=config.get("keep_alive", False),
                        working_dir=config.get("working_dir"),
                        env=config.get("env"),
                        last_heartbeat=self.registry.get_last_heartbeat(client_id, name),
                        command=config.get("command"),
                        args=config.get("args"),
                        package_name=config.get("package_name")
                    )
                    services_info.append(service_info)
            return services_info
        # 2. store传普通 client_id，只查该 client_id 下的服务
        if not agent_mode and id:
            if id == self.client_manager.main_client_id:
                # 已在上面聚合分支处理，这里直接返回空
                return services_info
            service_names = self.registry.get_all_service_names(id)
            for name in service_names:
                details = self.registry.get_service_details(id, name)
                config = self.config.get_service_config(name) or {}
                is_healthy = await self.orchestrator.is_service_healthy(name, id)
                service_info = ServiceInfo(
                    url=config.get("url", ""),
                    name=name,
                    transport_type=self._infer_transport_type(config),
                    status="healthy" if is_healthy else "unhealthy",
                    tool_count=details.get("tool_count", 0),
                    keep_alive=config.get("keep_alive", False),
                    working_dir=config.get("working_dir"),
                    env=config.get("env"),
                    last_heartbeat=self.registry.get_last_heartbeat(id, name),
                    command=config.get("command"),
                    args=config.get("args"),
                    package_name=config.get("package_name")
                )
                services_info.append(service_info)
            return services_info
        # 3. agent级别，聚合 agent_id 下所有 client_id 的服务；如果 id 不是 agent_id，尝试作为 client_id 查
        if agent_mode and id:
            client_ids = client_manager.get_agent_clients(id)
            if client_ids:
                for client_id in client_ids:
                    service_names = self.registry.get_all_service_names(client_id)
                    for name in service_names:
                        details = self.registry.get_service_details(client_id, name)
                        config = self.config.get_service_config(name) or {}
                        is_healthy = await self.orchestrator.is_service_healthy(name, client_id)
                        service_info = ServiceInfo(
                            url=config.get("url", ""),
                            name=name,
                            transport_type=self._infer_transport_type(config),
                            status="healthy" if is_healthy else "unhealthy",
                            tool_count=details.get("tool_count", 0),
                            keep_alive=config.get("keep_alive", False),
                            working_dir=config.get("working_dir"),
                            env=config.get("env"),
                            last_heartbeat=self.registry.get_last_heartbeat(client_id, name),
                            command=config.get("command"),
                            args=config.get("args"),
                            package_name=config.get("package_name")
                        )
                        services_info.append(service_info)
                return services_info
            else:
                service_names = self.registry.get_all_service_names(id)
                for name in service_names:
                    details = self.registry.get_service_details(id, name)
                    config = self.config.get_service_config(name) or {}
                    is_healthy = await self.orchestrator.is_service_healthy(name, id)
                    service_info = ServiceInfo(
                        url=config.get("url", ""),
                        name=name,
                        transport_type=self._infer_transport_type(config),
                        status="healthy" if is_healthy else "unhealthy",
                        tool_count=details.get("tool_count", 0),
                        keep_alive=config.get("keep_alive", False),
                        working_dir=config.get("working_dir"),
                        env=config.get("env"),
                        last_heartbeat=self.registry.get_last_heartbeat(id, name),
                        command=config.get("command"),
                        args=config.get("args"),
                        package_name=config.get("package_name")
                    )
                    services_info.append(service_info)
                return services_info
        return services_info

    async def list_tools(self, id: Optional[str] = None, agent_mode: bool = False) -> List[ToolInfo]:
        """
        列出工具列表：
        - store未传id 或 id==main_client：聚合 main_client 下所有 client_id 的工具
        - store传普通 client_id：只查该 client_id 下的工具
        - agent级别：聚合 agent_id 下所有 client_id 的工具；如果 id 不是 agent_id，尝试作为 client_id 查
        """
        from mcpstore.core.client_manager import ClientManager
        client_manager: ClientManager = self.client_manager
        tools = []
        # 1. store未传id 或 id==main_client，聚合 main_client 下所有 client_id 的工具
        if not agent_mode and (not id or id == self.client_manager.main_client_id):
            client_ids = client_manager.get_agent_clients(self.client_manager.main_client_id)
            for client_id in client_ids:
                tool_dicts = self.registry.get_all_tool_info(client_id)
                for tool in tool_dicts:
                    # 使用存储的键名作为显示名称（现在键名就是显示名称）
                    display_name = tool.get("name", "")
                    tools.append(ToolInfo(
                        name=display_name,
                        description=tool.get("description", ""),
                        service_name=tool.get("service_name", ""),
                        client_id=tool.get("client_id", ""),
                        inputSchema=tool.get("inputSchema", {})
                    ))
            return tools
        # 2. store传普通 client_id，只查该 client_id 下的工具
        if not agent_mode and id:
            if id == self.client_manager.main_client_id:
                return tools
            tool_dicts = self.registry.get_all_tool_info(id)
            for tool in tool_dicts:
                # 使用存储的键名作为显示名称（现在键名就是显示名称）
                display_name = tool.get("name", "")
                tools.append(ToolInfo(
                    name=display_name,
                    description=tool.get("description", ""),
                    service_name=tool.get("service_name", ""),
                    client_id=tool.get("client_id", ""),
                    inputSchema=tool.get("inputSchema", {})
                ))
            return tools
        # 3. agent级别，聚合 agent_id 下所有 client_id 的工具；如果 id 不是 agent_id，尝试作为 client_id 查
        if agent_mode and id:
            client_ids = client_manager.get_agent_clients(id)
            if client_ids:
                for client_id in client_ids:
                    tool_dicts = self.registry.get_all_tool_info(client_id)
                    for tool in tool_dicts:
                        # 使用存储的键名作为显示名称（现在键名就是显示名称）
                        display_name = tool.get("name", "")
                        tools.append(ToolInfo(
                            name=display_name,
                            description=tool.get("description", ""),
                            service_name=tool.get("service_name", ""),
                            client_id=tool.get("client_id", ""),
                            inputSchema=tool.get("inputSchema", {})
                        ))
                return tools
            else:
                tool_dicts = self.registry.get_all_tool_info(id)
                for tool in tool_dicts:
                    # 使用存储的键名作为显示名称（现在键名就是显示名称）
                    display_name = tool.get("name", "")
                    tools.append(ToolInfo(
                        name=display_name,
                        description=tool.get("description", ""),
                        service_name=tool.get("service_name", ""),
                        client_id=tool.get("client_id", ""),
                        inputSchema=tool.get("inputSchema", {})
                    ))
                return tools
        return tools

    async def use_tool(self, tool_name: str, args: Dict[str, Any]) -> Any:
        """
        使用工具（通用接口）
        
        Args:
            tool_name: 工具名称，格式为 service_toolname
            args: 工具参数
            
        Returns:
            Any: 工具执行结果
        """
        from mcpstore.core.models.tool import ToolExecutionRequest
        
        # 构造请求
        request = ToolExecutionRequest(
            tool_name=tool_name,
            args=args
        )
        
        # 处理工具请求
        return await self.process_tool_request(request)

    async def _add_service(self, service_names: List[str], agent_id: Optional[str]) -> bool:
        """内部方法：批量添加服务，store级别支持全量注册，agent级别支持指定服务注册"""
        # store级别
        if agent_id is None:
            if not service_names:
                # 全量注册
                resp = await self.register_json_service()
                return bool(resp and resp.service_names)
            else:
                # 支持单独添加服务
                resp = await self.register_json_service(service_names=service_names)
                return bool(resp and resp.service_names)
        # agent级别
        else:
            if service_names:
                resp = await self.register_json_service(client_id=agent_id, service_names=service_names)
                return bool(resp and resp.service_names)
            else:
                self.logger.warning("Agent级别添加服务时必须指定service_names")
                return False

    async def add_service(self, service_names: List[str], agent_id: Optional[str] = None) -> bool:
        context = self.for_agent(agent_id) if agent_id else self.for_store()
        return await context.add_service(service_names)

    def check_services(self, agent_id: Optional[str] = None) -> Dict[str, str]:
        """兼容旧版API"""
        context = self.for_agent(agent_id) if agent_id else self.for_store()
        return context.check_services()

    def show_mcpjson(self) -> Dict[str, Any]:
        """
        直接读取并返回 mcp.json 文件的内容
        
        Returns:
            Dict[str, Any]: mcp.json 文件的内容
        """
        return self.config.load_config()
