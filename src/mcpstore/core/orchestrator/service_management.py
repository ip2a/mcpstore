"""
MCPOrchestrator Service Management Module
Service management module - contains service registration, management and information retrieval
"""

import asyncio
import logging
from typing import Dict, List, Any, Optional, Tuple

from fastmcp import Client
from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)

class ServiceManagementMixin:
    """Service management mixin class"""

    async def register_agent_client(self, agent_id: str, config: Dict[str, Any] = None) -> Client:
        """
        Register a new client instance for agent

        Args:
            agent_id: Agent ID
            config: Optional configuration, if None use main_config

        Returns:
            Newly created Client instance
        """
        # Use main_config or provided config to create new client
        agent_config = config or self.main_config
        agent_client = Client(agent_config)

        # 存储agent_client
        self.agent_clients[agent_id] = agent_client
        logger.info(f"Registered agent client for {agent_id}")

        return agent_client

    def get_agent_client(self, agent_id: str) -> Optional[Client]:
        """
        获取agent的client实例

        Args:
            agent_id: 代理ID

        Returns:
            Client实例或None
        """
        return self.agent_clients.get(agent_id)

    async def filter_healthy_services(self, services: List[str], client_id: Optional[str] = None) -> List[str]:
        """
        过滤出健康的服务列表 - 使用生命周期管理器

        Args:
            services: 服务名列表
            client_id: 可选的客户端ID，用于多客户端环境

        Returns:
            List[str]: 健康的服务名列表
        """
        healthy_services = []
        agent_id = client_id or self.client_manager.global_agent_store_id

        for name in services:
            try:
                # 使用生命周期管理器获取服务状态
                service_state = self.lifecycle_manager.get_service_state(agent_id, name)

                # 🔧 修复：新服务（状态为None）也应该被处理
                if service_state is None:
                    healthy_services.append(name)
                    logger.debug(f"Service {name} has no state (new service), included in processable list")
                else:
                    # 健康状态和初始化状态的服务都被认为是可处理的
                    processable_states = [
                        ServiceConnectionState.HEALTHY,
                        ServiceConnectionState.WARNING,
                        ServiceConnectionState.INITIALIZING  # 新增：初始化状态也需要处理
                    ]
                    if service_state in processable_states:
                        healthy_services.append(name)
                        logger.debug(f"Service {name} is {service_state.value}, included in processable list")
                    else:
                        logger.debug(f"Service {name} is {service_state.value}, excluded from processable list")

            except Exception as e:
                logger.warning(f"Failed to check service state for {name}: {e}")
                continue

        logger.info(f"Filtered {len(healthy_services)} healthy services from {len(services)} total services")
        return healthy_services

    async def start_global_agent_store(self, config: Dict[str, Any]):
        """启动 global_agent_store 的 async with 生命周期，注册服务和工具（仅健康服务）"""
        # 获取健康的服务列表
        healthy_services = await self.filter_healthy_services(list(config.get("mcpServers", {}).keys()))
        
        # 创建一个新的配置，只包含健康的服务
        healthy_config = {
            "mcpServers": {
                name: config["mcpServers"][name]
                for name in healthy_services
            }
        }
        
        # 使用健康的配置注册服务
        await self.register_json_services(healthy_config, client_id="global_agent_store")
        # global_agent_store专属管理逻辑可在这里补充（如缓存、生命周期等）

    async def register_json_services(self, config: Dict[str, Any], client_id: str = None, agent_id: str = None):
        """
        @deprecated 此方法已废弃，请使用统一的add_service方法

        ⚠️ 警告：此方法已被统一注册架构替代，建议使用：
        - store.for_store().add_service_async() - Store级别注册
        - store.for_agent(agent_id).add_service_async() - Agent级别注册

        注册JSON配置中的服务（可用于global_agent_store或普通client）
        """


        # agent_id 兼容
        agent_key = agent_id or client_id or self.client_manager.global_agent_store_id
        try:
            # 获取健康的服务列表
            healthy_services = await self.filter_healthy_services(list(config.get("mcpServers", {}).keys()), client_id)
            
            # 创建一个新的配置，只包含健康的服务
            healthy_config = {
                "mcpServers": {
                    name: config["mcpServers"][name]
                    for name in healthy_services
                }
            }
            
            if not healthy_config["mcpServers"]:
                logger.warning(f"No healthy services found for client {agent_key}")
                return
            
            # 使用ConfigProcessor处理配置
            from mcpstore.core.config_processor import ConfigProcessor
            processed_config = ConfigProcessor.process_user_config_for_fastmcp(healthy_config)
            
            # 创建客户端
            client = Client(processed_config)
            
            # 连接并获取工具
            async with client:
                # 获取所有工具
                tools = await client.list_tools()
                
                # 按服务分组工具
                tools_by_service = {}
                for tool in tools:
                    # 从工具名推断服务名（这里需要更智能的逻辑）
                    service_name = self._infer_service_from_tool(tool.name, list(healthy_config["mcpServers"].keys()))
                    if service_name not in tools_by_service:
                        tools_by_service[service_name] = []
                    tools_by_service[service_name].append(tool)
                
                # 注册每个服务的工具
                for service_name, service_tools in tools_by_service.items():
                    try:
                        # 处理工具定义
                        processed_tools = []
                        for tool in service_tools:
                            try:
                                original_tool_name = tool.name
                                display_name = self._generate_display_name(original_tool_name, service_name)
                                
                                # 处理参数
                                parameters = {}
                                if hasattr(tool, 'inputSchema') and tool.inputSchema:
                                    if hasattr(tool.inputSchema, 'model_dump'):
                                        parameters = tool.inputSchema.model_dump()
                                    elif isinstance(tool.inputSchema, dict):
                                        parameters = tool.inputSchema
                                
                                # 构建工具定义
                                tool_def = {
                                    "type": "function",
                                    "function": {
                                        "name": original_tool_name,
                                        "display_name": display_name,
                                        "description": tool.description,
                                        "parameters": parameters,
                                        "service_name": service_name
                                    }
                                }
                                
                                processed_tools.append((display_name, tool_def))
                                
                            except Exception as e:
                                logger.error(f"Failed to process tool {tool.name}: {e}")
                                continue
                        
                        # 添加到Registry
                        self.registry.add_service(agent_key, service_name, client, processed_tools)
                        
                        # 标记长连接服务
                        service_config = healthy_config["mcpServers"].get(service_name, {})
                        if self._is_long_lived_service(service_config):
                            self.registry.mark_as_long_lived(agent_key, service_name)
                        
                        logger.info(f"Registered service '{service_name}' with {len(processed_tools)} tools for client '{agent_key}'")
                        
                    except Exception as e:
                        logger.error(f"Failed to register service {service_name}: {e}")
                        continue
                
                # 保存客户端配置到ClientManager
                self.client_manager.save_client_config(agent_key, processed_config)
                
                logger.info(f"Successfully registered {len(tools_by_service)} services with {len(tools)} total tools for client '{agent_key}'")
                
        except Exception as e:
            logger.error(f"Failed to register JSON services for client {agent_key}: {e}")
            raise

    def _infer_service_from_tool(self, tool_name: str, service_names: List[str]) -> str:
        """从工具名推断服务名"""
        # 简单的推断逻辑：查找工具名中包含的服务名
        for service_name in service_names:
            if service_name.lower() in tool_name.lower():
                return service_name
        
        # 如果没有匹配，返回第一个服务名（假设单服务配置）
        return service_names[0] if service_names else "unknown_service"

    def create_client_config_from_names(self, service_names: list) -> Dict[str, Any]:
        """
        根据服务名列表，从 mcp.json 生成新的 client config
        """
        all_services = self.mcp_config.load_config().get("mcpServers", {})
        selected = {name: all_services[name] for name in service_names if name in all_services}
        return {"mcpServers": selected}

    async def remove_service(self, service_name: str, agent_id: str = None):
        """移除服务并处理生命周期状态"""
        try:
            # 🔧 修复：更安全的agent_id处理
            if agent_id is None:
                if not hasattr(self.client_manager, 'global_agent_store_id'):
                    logger.error("No agent_id provided and global_agent_store_id not available")
                    raise ValueError("Agent ID is required for service removal")
                agent_key = self.client_manager.global_agent_store_id
                logger.debug(f"Using global_agent_store_id: {agent_key}")
            else:
                agent_key = agent_id
                logger.debug(f"Using provided agent_id: {agent_key}")

            # 🔧 修复：检查服务是否存在于生命周期管理器中
            current_state = self.lifecycle_manager.get_service_state(agent_key, service_name)
            if current_state is None:
                logger.warning(f"Service {service_name} not found in lifecycle manager for agent {agent_key}")
                # 检查是否存在于注册表中
                if agent_key not in self.registry.sessions or service_name not in self.registry.sessions[agent_key]:
                    logger.warning(f"Service {service_name} not found in registry for agent {agent_key}, skipping removal")
                    return
                else:
                    logger.info(f"Service {service_name} found in registry but not in lifecycle manager, proceeding with cleanup")

            if current_state:
                logger.info(f"Removing service {service_name} from agent {agent_key} (current state: {current_state.value})")
            else:
                logger.info(f"Removing service {service_name} from agent {agent_key} (no lifecycle state)")

            # 🔧 修复：安全地调用各个组件的移除方法
            try:
                # 通知生命周期管理器开始优雅断连（如果服务存在于生命周期管理器中）
                if current_state:
                    await self.lifecycle_manager.graceful_disconnect(agent_key, service_name, "user_requested")
            except Exception as e:
                logger.warning(f"Error during graceful disconnect: {e}")

            try:
                # 从内容监控中移除
                self.content_manager.remove_service_from_monitoring(agent_key, service_name)
            except Exception as e:
                logger.warning(f"Error removing from content monitoring: {e}")

            try:
                # 从注册表中移除服务
                self.registry.remove_service(agent_key, service_name)
            except Exception as e:
                logger.warning(f"Error removing from registry: {e}")

            try:
                # 移除生命周期数据
                self.lifecycle_manager.remove_service(agent_key, service_name)
            except Exception as e:
                logger.warning(f"Error removing lifecycle data: {e}")

            logger.info(f"Service {service_name} removal completed for agent {agent_key}")

        except Exception as e:
            logger.error(f"Error removing service {service_name}: {e}")
            import traceback
            logger.error(f"Traceback: {traceback.format_exc()}")
            raise

    def get_session(self, service_name: str, agent_id: str = None):
        agent_key = agent_id or self.client_manager.global_agent_store_id
        return self.registry.get_session(agent_key, service_name)

    def get_tools_for_service(self, service_name: str, agent_id: str = None):
        agent_key = agent_id or self.client_manager.global_agent_store_id
        return self.registry.get_tools_for_service(agent_key, service_name)

    def get_all_service_names(self, agent_id: str = None):
        agent_key = agent_id or self.client_manager.global_agent_store_id
        return self.registry.get_all_service_names(agent_key)

    def get_all_tool_info(self, agent_id: str = None):
        agent_key = agent_id or self.client_manager.global_agent_store_id
        return self.registry.get_all_tool_info(agent_key)

    def get_service_details(self, service_name: str, agent_id: str = None):
        agent_key = agent_id or self.client_manager.global_agent_store_id
        return self.registry.get_service_details(agent_key, service_name)

    def update_service_health(self, service_name: str, agent_id: str = None):
        """
        ⚠️ 已废弃：此方法已被ServiceLifecycleManager替代
        """
        logger.debug(f"update_service_health is deprecated for service: {service_name}")
        pass

    def get_last_heartbeat(self, service_name: str, agent_id: str = None):
        """
        ⚠️ 已废弃：此方法已被ServiceLifecycleManager替代
        """
        logger.debug(f"get_last_heartbeat is deprecated for service: {service_name}")
        return None

    def has_service(self, service_name: str, agent_id: str = None):
        agent_key = agent_id or self.client_manager.global_agent_store_id
        return self.registry.has_service(agent_key, service_name)

    async def restart_service(self, service_name: str, agent_id: str = None) -> bool:
        """
        重启服务 - 重置为初始化状态，让生命周期管理器重新处理

        Args:
            service_name: 服务名称
            agent_id: Agent ID，如果为None则使用global_agent_store_id

        Returns:
            bool: 重启是否成功
        """
        try:
            agent_key = agent_id or self.client_manager.global_agent_store_id

            logger.info(f"🔄 [RESTART_SERVICE] Starting restart for service '{service_name}' (agent: {agent_key})")

            # 检查服务是否存在
            if not self.registry.has_service(agent_key, service_name):
                logger.warning(f"⚠️ [RESTART_SERVICE] Service '{service_name}' not found in registry")
                return False

            # 获取服务元数据
            metadata = self.registry.get_service_metadata(agent_key, service_name)
            if not metadata:
                logger.error(f"❌ [RESTART_SERVICE] No metadata found for service '{service_name}'")
                return False

            # 重置服务状态为 INITIALIZING
            self.registry.set_service_state(agent_key, service_name, ServiceConnectionState.INITIALIZING)
            logger.debug(f"🔄 [RESTART_SERVICE] Set state to INITIALIZING for '{service_name}'")

            # 重置元数据
            from datetime import datetime
            metadata.consecutive_failures = 0
            metadata.consecutive_successes = 0
            metadata.reconnect_attempts = 0
            metadata.error_message = None
            metadata.state_entered_time = datetime.now()
            metadata.next_retry_time = None

            # 更新元数据到注册表
            self.registry.set_service_metadata(agent_key, service_name, metadata)
            logger.debug(f"🔄 [RESTART_SERVICE] Reset metadata for '{service_name}'")

            # 如果有生命周期管理器，触发初始化
            if hasattr(self, 'lifecycle_manager') and self.lifecycle_manager:
                init_success = self.lifecycle_manager.initialize_service(agent_key, service_name, metadata.service_config)
                logger.debug(f"🔄 [RESTART_SERVICE] Triggered lifecycle initialization for '{service_name}': {init_success}")

            logger.info(f"✅ [RESTART_SERVICE] Successfully restarted service '{service_name}'")
            return True

        except Exception as e:
            logger.error(f"❌ [RESTART_SERVICE] Failed to restart service '{service_name}': {e}")
            return False

    def _generate_display_name(self, original_tool_name: str, service_name: str) -> str:
        """
        生成用户友好的工具显示名称

        Args:
            original_tool_name: 原始工具名称
            service_name: 服务名称

        Returns:
            用户友好的显示名称
        """
        try:
            from mcpstore.core.registry.tool_resolver import ToolNameResolver
            resolver = ToolNameResolver()
            return resolver.create_user_friendly_name(service_name, original_tool_name)
        except Exception as e:
            logger.warning(f"Failed to generate display name for {original_tool_name}: {e}")
            # 回退到简单格式
            return f"{service_name}_{original_tool_name}"

    def _is_long_lived_service(self, service_config: Dict[str, Any]) -> bool:
        """
        判断是否为长连接服务

        Args:
            service_config: 服务配置

        Returns:
            是否为长连接服务
        """
        # STDIO服务默认是长连接（keep_alive=True）
        if "command" in service_config:
            return service_config.get("keep_alive", True)

        # HTTP服务通常也是长连接
        if "url" in service_config:
            return True

        return False

    def get_service_status(self, service_name: str, client_id: str = None) -> dict:
        """
        获取服务状态信息 - 纯缓存查询，不执行任何业务逻辑

        Args:
            service_name: 服务名称
            client_id: 客户端ID（可选，默认使用global_agent_store_id）

        Returns:
            dict: 包含状态信息的字典
            {
                "service_name": str,
                "status": str,  # "healthy", "warning", "disconnected", "unknown", etc.
                "healthy": bool,
                "last_check": float,  # timestamp
                "response_time": float,
                "error": str (可选),
                "client_id": str
            }
        """
        try:
            agent_key = client_id or self.client_manager.global_agent_store_id

            # 从缓存获取服务状态
            state = self.registry.get_service_state(agent_key, service_name)
            metadata = self.registry.get_service_metadata(agent_key, service_name)

            # 构建状态响应
            status_response = {
                "service_name": service_name,
                "client_id": agent_key
            }

            if state:
                status_response["status"] = state.value
                # 判断是否健康：HEALTHY 和 WARNING 都算健康
                from mcpstore.core.models.service import ServiceConnectionState
                status_response["healthy"] = state in [
                    ServiceConnectionState.HEALTHY,
                    ServiceConnectionState.WARNING
                ]
            else:
                status_response["status"] = "unknown"
                status_response["healthy"] = False

            if metadata:
                status_response["last_check"] = metadata.last_health_check.timestamp() if metadata.last_health_check else None
                status_response["response_time"] = metadata.last_response_time
                status_response["error"] = metadata.error_message
                status_response["consecutive_failures"] = metadata.consecutive_failures
                status_response["state_entered_time"] = metadata.state_entered_time.timestamp() if metadata.state_entered_time else None
            else:
                status_response["last_check"] = None
                status_response["response_time"] = None
                status_response["error"] = None
                status_response["consecutive_failures"] = 0
                status_response["state_entered_time"] = None

            logger.debug(f"Retrieved cached status for service {service_name}: {status_response['status']}")
            return status_response

        except Exception as e:
            logger.error(f"Failed to get service status from cache for {service_name}: {e}")
            return {
                "service_name": service_name,
                "status": "error",
                "healthy": False,
                "last_check": None,
                "response_time": None,
                "error": f"Cache query failed: {str(e)}",
                "client_id": client_id or (self.client_manager.global_agent_store_id if hasattr(self, 'client_manager') else "unknown"),
                "consecutive_failures": 0,
                "state_entered_time": None
            }
