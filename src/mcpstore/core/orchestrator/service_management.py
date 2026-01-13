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

    # tools_snapshot 和 _get_all_tools_from_cache 已删除
    # 所有工具数据直接从 pykv 读取，不使用快照
    # 参见 tool_operations.py 中的 list_tools_async 方法

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
            ServiceConnectionState.DEGRADED,
            ServiceConnectionState.STARTUP,
        ]
        healthy_services: List[str] = []
        agent_id = self.client_manager.global_agent_store_id

        for name in config.get("mcpServers", {}).keys():
            state = await self.registry._service_state_service.get_service_state_async(agent_id, name)

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
            current_state = await self.registry._service_state_service.get_service_state_async(agent_key, service_name)
            if current_state is None:
                logger.warning(f"Service {service_name} not found in lifecycle manager for agent {agent_key}")
                # Check if it exists in the registry（使用异步 API）
                if not await self.registry.has_service_async(agent_key, service_name):
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
                # Remove service from registry（使用异步版本）
                await self.registry.remove_service_async(agent_key, service_name)

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
                    # 使用新的状态管理器删除服务状态
                    state_manager = self.registry._cache_state_manager
                    await state_manager.delete_service_status(service_global_name)
                    logger.info(
                        f"Service status cleanup successful: service_global_name={service_global_name}"
                    )
                else:
                    logger.debug(
                        f"Cannot get service global name, skipping status cleanup: "
                        f"agent_id={agent_key}, service_name={service_name}"
                    )
            except Exception as e:
                logger.warning(
                    f"Service status cleanup failed (does not affect service removal): "
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

            # Check if service exists（使用异步 API）
            if not await self.registry.has_service_async(agent_key, service_name):
                logger.warning(f"[RESTART_SERVICE] Service '{service_name}' not found in registry")
                return False

            # 从 pykv 异步获取服务元数据
            metadata = await self.registry.get_service_metadata_async(agent_key, service_name)
            if not metadata:
                logger.error(f" [RESTART_SERVICE] No metadata found for service '{service_name}'")
                raise RuntimeError(f"No metadata found for service '{service_name}'")

            # Reset service state to STARTUP（通过 LifecycleManager 统一入口）
            await self.lifecycle_manager._transition_state(
                agent_id=agent_key,
                service_name=service_name,
                new_state=ServiceConnectionState.STARTUP,
                reason="restart_service",
                source="ServiceManagement",
            )
            logger.debug(f" [RESTART_SERVICE] Set state to STARTUP for '{service_name}'")

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
                        initial_state="startup"
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

    async def get_service_status_async(self, service_name: str, client_id: str = None) -> dict:
        """
        异步获取服务状态信息 - 从 pykv 读取

        Args:
            service_name: 服务名称
            client_id: Client ID（可选，默认使用 global_agent_store_id）

        Returns:
            dict: 包含状态信息的字典
            {
                "service_name": str,
                "status": str,  # "healthy", "degraded", "disconnected", "unknown", etc.
                "healthy": bool,
                "last_check": float,  # timestamp
                "response_time": float,
                "error": str (optional),
                "client_id": str
            }
        """
        try:
            agent_key = client_id or self.client_manager.global_agent_store_id

            # 统一从完整信息读取，避免“关系存在但状态缺失”导致的 unknown
            complete_info = await self.registry.get_complete_service_info_async(agent_key, service_name)
            state = complete_info.get("state") if complete_info else None
            metadata = complete_info.get("state_metadata") if complete_info else None
            client_id_resolved = complete_info.get("client_id") if complete_info else agent_key

            # Build status response
            status_response = {
                "service_name": service_name,
                "client_id": client_id_resolved
            }

            if isinstance(state, str):
                try:
                    state = ServiceConnectionState(state)
                except ValueError:
                    state = None

            if state:
                status_response["status"] = state.value
                status_response["healthy"] = state in [
                    ServiceConnectionState.HEALTHY,
                    ServiceConnectionState.DEGRADED,
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
