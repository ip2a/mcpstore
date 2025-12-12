"""
MCPOrchestrator Service Management Module
Service management module - contains service registration, management and information retrieval
"""

import logging
from typing import Dict, List, Any, Optional

from fastmcp import Client

from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)

class ServiceManagementMixin:
    """Service management mixin class"""

    async def _get_all_tools_from_cache(self, agent_id: str) -> Dict[str, Dict[str, Any]]:
        """
        从三层缓存架构获取指定 Agent 的所有工具
        
        使用新的缓存架构：
        1. 从关系层获取 Agent 的所有服务
        2. 从关系层获取每个服务的工具列表
        3. 从实体层批量获取工具定义
        
        Args:
            agent_id: Agent ID
            
        Returns:
            工具字典 {tool_global_name: tool_definition}
        """
        tools_dict: Dict[str, Dict[str, Any]] = {}
        
        # 1. 获取 Agent 的所有服务关系
        services = await self.registry._relation_manager.get_agent_services(agent_id)
        
        if not services:
            logger.debug(f"[TOOLS] Agent {agent_id} 没有服务")
            return tools_dict
        
        # 2. 收集所有工具全局名称
        all_tool_global_names = []
        for service in services:
            service_global_name = service.get("service_global_name")
            if not service_global_name:
                continue
            
            # 获取服务的工具关系
            tool_relations = await self.registry._relation_manager.get_service_tools(
                service_global_name
            )
            
            for tool_rel in tool_relations:
                tool_global_name = tool_rel.get("tool_global_name")
                if tool_global_name:
                    all_tool_global_names.append(tool_global_name)
        
        if not all_tool_global_names:
            logger.debug(f"[TOOLS] Agent {agent_id} 没有工具")
            return tools_dict
        
        # 3. 批量获取工具实体
        tool_entities = await self.registry._tool_manager.get_many_tools(
            all_tool_global_names
        )
        
        # 4. 构建工具字典
        for i, entity in enumerate(tool_entities):
            if entity is None:
                continue
            
            tool_global_name = all_tool_global_names[i]
            
            # 转换为标准格式
            tools_dict[tool_global_name] = {
                "name": entity.tool_original_name,
                "display_name": entity.tool_original_name,
                "original_name": entity.tool_original_name,
                "description": entity.description,
                "inputSchema": entity.input_schema,
                "parameters": entity.input_schema,
                "service_name": entity.service_original_name,
                "service_global_name": entity.service_global_name,
                "tool_global_name": entity.tool_global_name,
                "source_agent": entity.source_agent
            }
        
        logger.debug(
            f"[TOOLS] 获取到 {len(tools_dict)} 个工具: agent_id={agent_id}"
        )
        
        return tools_dict

    async def tools_snapshot(self, agent_id: Optional[str] = None) -> List[Any]:
        """Public API: read tools directly from pyvk and project agent view.

        - Always read global tools from pyvk (single source of truth).
        - If agent_id provided, project the global services to agent-local names using
          the current mapping state.
        - Real-time data, no snapshot caching.
        """
        try:
            global_agent_id = self.client_manager.global_agent_store_id

            # 1. Read all tools from pyvk using new cache architecture
            tools_dict = await self._get_all_tools_from_cache(global_agent_id)
            logger.debug(f"[TOOLS] Loaded {len(tools_dict)} tools from pyvk for global_agent_id={global_agent_id}")

            # 2. Get all service names
            service_names = self.registry._service_state_service.get_all_service_names(global_agent_id)

            # 3. Build services index: service_name -> [tool_info, ...]
            services_index: Dict[str, List[Dict[str, Any]]] = {}
            for service_name in service_names:
                tool_names = self.registry.get_tools_for_service(global_agent_id, service_name)
                if not tool_names:
                    services_index[service_name] = []
                    continue

                items: List[Dict[str, Any]] = []
                for tool_name in tool_names:
                    tool_def = tools_dict.get(tool_name)
                    if not tool_def:
                        continue

                    # Extract tool info from definition (inline logic)
                    # Get Client ID using public method
                    client_id = self.registry.get_service_client_id(global_agent_id, service_name) if service_name else None

                    # Handle different tool definition formats
                    if "function" in tool_def:
                        function_data = tool_def["function"]
                        tool_info = {
                            'name': tool_name,
                            'display_name': function_data.get('display_name', tool_name),
                            'original_name': function_data.get('name', tool_name),
                            'description': function_data.get('description', ''),
                            'inputSchema': function_data.get('parameters', {}),
                            'service_name': service_name,
                            'client_id': client_id
                        }
                    else:
                        tool_info = {
                            'name': tool_name,
                            'display_name': tool_def.get('display_name', tool_name),
                            'original_name': tool_def.get('name', tool_name),
                            'description': tool_def.get('description', ''),
                            'inputSchema': tool_def.get('parameters', {}),
                            'service_name': service_name,
                            'client_id': client_id
                        }

                    if not tool_info:
                        continue

                    # Normalize to standard format
                    full_name = tool_info.get("name", tool_name)
                    item = {
                        "name": full_name,
                        "display_name": tool_info.get("display_name", tool_info.get("original_name", full_name.split(f"{service_name}_", 1)[-1] if isinstance(full_name, str) else full_name)),
                        "description": tool_info.get("description", ""),
                        "service_name": service_name,
                        "client_id": tool_info.get("client_id"),
                        "inputSchema": tool_info.get("inputSchema", {}),
                        "original_name": tool_info.get("original_name", tool_info.get("name", tool_name))
                    }
                    items.append(item)

                services_index[service_name] = items

            # 4. Flatten global tools
            flat_global: List[Dict[str, Any]] = []
            for svc, items in services_index.items():
                if not items:
                    continue
                for it in items:
                    entry = dict(it)
                    entry["service_name"] = svc
                    flat_global.append(entry)

            logger.debug(f"[TOOLS] Built {len(flat_global)} global tools from {len(services_index)} services")

            # 5. If no agent_id, return global tools directly
            if not agent_id:
                return flat_global

            # 6. Real-time agent projection: global -> local service names
            # 从关系层获取 Agent 的服务映射
            agent_services = await self.registry._relation_manager.get_agent_services(agent_id)
            
            # 构建 global_name -> local_name 的反向映射
            reverse_map: Dict[str, str] = {}
            for svc in agent_services:
                global_name = svc.get("service_global_name")
                local_name = svc.get("service_original_name")
                if global_name and local_name:
                    reverse_map[global_name] = local_name

            logger.debug(f"[TOOLS] Agent projection for agent_id={agent_id}: reverse_map={reverse_map}")

            # 7. Project tools to agent view
            projected: List[Dict[str, Any]] = []
            skipped_count = 0
            for item in flat_global:
                gsvc = item.get("service_name")
                lsvc = reverse_map.get(gsvc)
                if not lsvc:
                    # Skip services without mapping for this agent
                    skipped_count += 1
                    continue

                new_item = dict(item)
                new_item["service_name"] = lsvc

                # Rewrite tool name to use local service prefix
                name = new_item.get("name")
                if isinstance(name, str):
                    if name.startswith(f"{gsvc}_"):
                        suffix = name[len(gsvc) + 1:]
                        new_item["name"] = f"{lsvc}_{suffix}"
                    elif name.startswith(f"{gsvc}__"):
                        suffix = name[len(gsvc) + 2:]
                        new_item["name"] = f"{lsvc}_{suffix}"

                projected.append(new_item)

            logger.debug(f"[TOOLS] Agent view: {len(projected)} tools projected, {skipped_count} skipped")
            return projected

        except Exception as e:
            logger.error(f"Failed to get tools snapshot: {e}")
            return []

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

        # Store agent_client
        self.agent_clients[agent_id] = agent_client
        logger.debug(f"Registered agent client for {agent_id}")

        return agent_client

    def get_agent_client(self, agent_id: str) -> Optional[Client]:
        """
        Get client instance for agent

        Args:
            agent_id: Agent ID

        Returns:
            Client instance or None
        """
        return self.agent_clients.get(agent_id)

    async def start_global_agent_store(self, config: Dict[str, Any]):
        """Start global_agent_store async with lifecycle, register services and tools (healthy services only)"""
        # Get list of healthy services
        # 直接查询健康服务（基于当前生命周期状态）
        processable_states = [
            ServiceConnectionState.HEALTHY,
            ServiceConnectionState.WARNING,
            ServiceConnectionState.INITIALIZING,
        ]
        healthy_services: List[str] = []
        agent_id = self.client_manager.global_agent_store_id

        for name in config.get("mcpServers", {}).keys():
            state = self.registry._service_state_service.get_service_state(agent_id, name)

            # 新服务（state=None）也应纳入处理范围
            if state is None or state in processable_states:
                healthy_services.append(name)

        # Create new configuration containing only healthy services
        healthy_config = {
            "mcpServers": {
                name: config["mcpServers"][name]
                for name in healthy_services
            }
        }

        # Use unified registration path (replacing deprecated register_json_services)
        try:
            if self._context_factory:
                context = self._context_factory()
                await context.add_service_async(healthy_config)
            else:
                logger.warning("Orchestrator context factory not available; skipping auto registration pipeline")
        except Exception as e:
            logger.error(f"Failed to register healthy services via add_service_async: {e}")

    # register_json_services removed (Deprecated)

    def _infer_service_from_tool(self, tool_name: str, service_names: List[str]) -> str:
        """Infer service name from tool name"""
        # Simple inference logic: find service name contained in tool name
        for service_name in service_names:
            if service_name.lower() in tool_name.lower():
                return service_name

        # If no match, return first service name (assuming single service configuration)
        return service_names[0] if service_names else "unknown_service"

    def create_client_config_from_names(self, service_names: list) -> Dict[str, Any]:
        """
        Generate new client config from mcp.json based on service name list
        """
        all_services = self.mcp_config.load_config().get("mcpServers", {})
        selected = {name: all_services[name] for name in service_names if name in all_services}
        return {"mcpServers": selected}

    async def remove_service(self, service_name: str, agent_id: str = None):
        """
        Remove service and handle lifecycle state
        
        Args:
            service_name: 服务名称
            agent_id: Agent ID（可选）
        """
        try:
            #  Fix: safer agent_id handling
            if agent_id is None:
                if not hasattr(self.client_manager, 'global_agent_store_id'):
                    logger.error("No agent_id provided and global_agent_store_id not available")
                    raise ValueError("Agent ID is required for service removal")
                agent_key = self.client_manager.global_agent_store_id
                logger.debug(f"Using global_agent_store_id: {agent_key}")
            else:
                agent_key = agent_id
                logger.debug(f"Using provided agent_id: {agent_key}")

            # 🆕 Event-driven architecture: directly check service status from registry
            current_state = self.registry._service_state_service.get_service_state(agent_key, service_name)
            if current_state is None:
                logger.warning(f"Service {service_name} not found in lifecycle manager for agent {agent_key}")
                # Check if it exists in the registry
                if not self.registry.has_service(agent_key, service_name):
                    logger.warning(f"Service {service_name} not found in registry for agent {agent_key}, skipping removal")
                    return
                else:
                    logger.debug(f"Service {service_name} found in registry but not in lifecycle, cleaning up")

            if current_state:
                logger.debug(f"Removing service {service_name} from agent {agent_key} (state: {current_state.value})")
            else:
                logger.debug(f"Removing service {service_name} from agent {agent_key} (no lifecycle state)")

            #  Fix: safely call removal methods for each component
            try:
                # Notify lifecycle manager to start graceful disconnect (if service exists in lifecycle manager)
                if current_state:
                    await self.lifecycle_manager.graceful_disconnect(agent_key, service_name, "user_requested")
            except Exception as e:
                logger.warning(f"Error during graceful disconnect: {e}")

            try:
                # Remove from content monitoring
                self.content_manager.remove_service_from_monitoring(agent_key, service_name)
            except Exception as e:
                logger.warning(f"Error removing from content monitoring: {e}")

            try:
                # Remove service from registry
                self.registry.remove_service(agent_key, service_name)

                # Cancel health monitoring (if exists)
                try:
                    if self.container:
                        hm = getattr(self.container, 'health_monitor', None)
                        if hm and hasattr(hm, '_health_check_tasks'):
                            task_key = (agent_key, service_name)
                            task = hm._health_check_tasks.pop(task_key, None)
                            if task and not task.done():
                                task.cancel()
                            logger.debug(f"[HEALTH] Unwatched removed service: {service_name} (agent={agent_key})")
                except Exception as e:
                    logger.debug(f"[HEALTH] Unwatch removed service failed: {e}")
            except Exception as e:
                logger.warning(f"Error removing from registry: {e}")

            try:
                # Remove lifecycle data
                self.lifecycle_manager.remove_service(agent_key, service_name)
            except Exception as e:
                logger.warning(f"Error removing lifecycle data: {e}")

            # 清理服务状态（使用 StateManager）
            try:
                # 获取服务的全局名称
                global_agent_store_id = self.client_manager.global_agent_store_id
                if agent_key != global_agent_store_id:
                    # Agent 模式：需要获取全局服务名
                    service_global_name = self.registry.get_global_name_from_agent_service(
                        agent_key, service_name
                    )
                else:
                    # Store 模式：服务名就是全局名称
                    service_global_name = service_name
                
                if service_global_name:
                    # 使用 StateManager 删除服务状态
                    state_manager = self.registry._state_manager
                    await state_manager.delete_service_status(service_global_name)
                    logger.info(
                        f"服务状态清理成功: service_global_name={service_global_name}"
                    )
                else:
                    logger.debug(
                        f"无法获取服务全局名称，跳过状态清理: "
                        f"agent_id={agent_key}, service_name={service_name}"
                    )
            except Exception as e:
                logger.warning(
                    f"服务状态清理失败（不影响服务移除）: "
                    f"agent_id={agent_key}, service_name={service_name}, error={e}"
                )

            logger.debug(f"Service removal completed: {service_name} from agent {agent_key}")

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

    # 🆕 Event-driven architecture: the following methods have been deprecated and removed
    # - update_service_health: replaced by ServiceLifecycleManager
    # - get_last_heartbeat: replaced by ServiceLifecycleManager

    def has_service(self, service_name: str, agent_id: str = None):
        agent_key = agent_id or self.client_manager.global_agent_store_id
        return self.registry.has_service(agent_key, service_name)

    async def restart_service(self, service_name: str, agent_id: str = None) -> bool:
        """
        Restart service - reset to initializing state, let lifecycle manager reprocess

        Args:
            service_name: Service name
            agent_id: Agent ID, if None then use global_agent_store_id

        Returns:
            bool: Whether restart was successful
        """
        try:
            agent_key = agent_id or self.client_manager.global_agent_store_id

            logger.debug(f"Restarting service {service_name} for agent {agent_key}")

            # Check if service exists
            if not self.registry.has_service(agent_key, service_name):
                logger.warning(f"[RESTART_SERVICE] Service '{service_name}' not found in registry")
                return False

            # Get service metadata
            metadata = self.registry.get_service_metadata(agent_key, service_name)
            if not metadata:
                logger.error(f" [RESTART_SERVICE] No metadata found for service '{service_name}'")
                return False

            # Reset service state to INITIALIZING（通过 LifecycleManager 统一入口）
            await self.lifecycle_manager._transition_state(
                agent_id=agent_key,
                service_name=service_name,
                new_state=ServiceConnectionState.INITIALIZING,
                reason="restart_service",
                source="ServiceManagement",
            )
            logger.debug(f" [RESTART_SERVICE] Set state to INITIALIZING for '{service_name}'")

            # Reset metadata
            from datetime import datetime
            metadata.consecutive_failures = 0
            metadata.consecutive_successes = 0
            metadata.reconnect_attempts = 0
            metadata.error_message = None
            metadata.state_entered_time = datetime.now()
            metadata.next_retry_time = None

            # Update metadata to registry
            self.registry.set_service_metadata(agent_key, service_name, metadata)
            logger.debug(f" [RESTART_SERVICE] Reset metadata for '{service_name}'")

            # Event-driven architecture: directly publish ServiceInitialized, let ConnectionManager handle connection
            try:
                from mcpstore.core.events.service_events import ServiceInitialized
                # Prefer container.event_bus; otherwise fallback to orchestrator.event_bus
                bus = None
                bus_source = None
                if self.container:
                    bus = getattr(self.container, 'event_bus', None)
                    bus_source = 'container.event_bus' if bus else None
                if not bus:
                    bus = getattr(self, 'event_bus', None)
                    bus_source = bus_source or ('orchestrator.event_bus' if bus else None)

                # Diagnostics: compare bus identities
                try:
                    container_bus = getattr(self.container, 'event_bus', None) if self.container else None
                    orchestrator_bus = getattr(self, 'event_bus', None)
                    logger.debug(
                        f" [RESTART_SERVICE] bus_diag chosen={hex(id(bus)) if bus else 'None'} "
                        f"container={hex(id(container_bus)) if container_bus else 'None'} "
                        f"orchestrator={hex(id(orchestrator_bus)) if orchestrator_bus else 'None'}"
                    )
                except Exception as e:
                    logger.debug(f" [RESTART_SERVICE] bus_diag error: {e}")

                if bus:
                    initialized_event = ServiceInitialized(
                        agent_id=agent_key,
                        service_name=service_name,
                        initial_state="initializing"
                    )
                    await bus.publish(initialized_event, wait=True)
                    logger.debug(f" [RESTART_SERVICE] Published ServiceInitialized for '{service_name}' via {bus_source}")

                    # Add one-time health check request to ensure quick convergence after initialization (no need to wait for periodic heartbeat)
                    from mcpstore.core.events.service_events import HealthCheckRequested
                    health_check_event = HealthCheckRequested(
                        agent_id=agent_key,
                        service_name=service_name
                    )
                    await bus.publish(health_check_event, wait=True)
                    logger.debug(f" [RESTART_SERVICE] Published HealthCheckRequested for '{service_name}' via {bus_source}")
                else:
                    logger.warning(" [RESTART_SERVICE] EventBus not available (neither orchestrator nor store.container); cannot publish ServiceInitialized")
            except Exception as pub_err:
                logger.warning(f" [RESTART_SERVICE] Failed to publish ServiceInitialized for '{service_name}': {pub_err}")

            logger.info(f"Service restarted successfully: {service_name}")
            return True

        except Exception as e:
            logger.error(f" [RESTART_SERVICE] Failed to restart service '{service_name}': {e}")
            return False

    def _generate_display_name(self, original_tool_name: str, service_name: str) -> str:
        """
        Generate user-friendly tool display name

        Args:
            original_tool_name: Original tool name
            service_name: Service name

        Returns:
            User-friendly display name
        """
        try:
            from mcpstore.core.registry.tool_resolver import ToolNameResolver
            resolver = ToolNameResolver()
            return resolver.create_user_friendly_name(service_name, original_tool_name)
        except Exception as e:
            logger.warning(f"Failed to generate display name for {original_tool_name}: {e}")
            # Fallback to simple format
            return f"{service_name}_{original_tool_name}"

    def _is_long_lived_service(self, service_config: Dict[str, Any]) -> bool:
        """
        Determine if it's a long connection service

        Args:
            service_config: Service configuration

        Returns:
            Whether it's a long connection service
        """
        # STDIO services are long connections by default (keep_alive=True)
        if "command" in service_config:
            return service_config.get("keep_alive", True)

        # HTTP services are usually also long connections
        if "url" in service_config:
            return True

        return False

    def get_service_status(self, service_name: str, client_id: str = None) -> dict:
        """
        Get service status information - pure cache query, no business logic execution

        Args:
            service_name: Service name
            client_id: Client ID (optional, default uses global_agent_store_id)

        Returns:
            dict: Dictionary containing status information
            {
                "service_name": str,
                "status": str,  # "healthy", "warning", "disconnected", "unknown", etc.
                "healthy": bool,
                "last_check": float,  # timestamp
                "response_time": float,
                "error": str (optional),
                "client_id": str
            }
        """
        try:
            agent_key = client_id or self.client_manager.global_agent_store_id

            # Get service status from cache
            state = self.registry._service_state_service.get_service_state(agent_key, service_name)
            metadata = self.registry._service_state_service.get_service_metadata(agent_key, service_name)

            # Build status response
            status_response = {
                "service_name": service_name,
                "client_id": agent_key
            }

            if state:
                status_response["status"] = state.value
                # Determine if healthy: both HEALTHY and WARNING are considered healthy
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

            logger.info(f"[GET_STATUS] service='{service_name}' agent_key='{agent_key}' status='{status_response.get('status')}' healthy={status_response.get('healthy')} last_check={status_response.get('last_check')} resp_time={status_response.get('response_time')} cf={status_response.get('consecutive_failures')}")
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
