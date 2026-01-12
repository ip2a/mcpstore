"""
Connection Manager - Responsible for actual service connections

Responsibilities:
1. Listen to ServiceInitialized events, trigger connections
2. Execute actual service connections (local/remote)
3. Publish ServiceConnected/ServiceConnectionFailed events
"""

import asyncio
import logging
from typing import Dict, Any, Tuple, List

from mcpstore.core.configuration.config_processor import ConfigProcessor
from mcpstore.core.events.event_bus import EventBus
from mcpstore.core.events.service_events import (
    ServiceInitialized, ServiceConnectionRequested,
    ServiceConnected, ServiceConnectionFailed
)

logger = logging.getLogger(__name__)


class ConnectionManager:
    """
    Connection Manager

    Responsibilities:
    1. Listen to ServiceInitialized events, trigger connections
    2. Execute actual service connections (local/remote)
    3. Publish ServiceConnected/ServiceConnectionFailed events
    """

    def __init__(
        self,
        event_bus: EventBus,
        registry: 'CoreRegistry',
        config_processor: 'ConfigProcessor',
        local_service_manager: 'LocalServiceManagerAdapter',
        http_timeout_seconds: float = 10.0
    ):
        self._event_bus = event_bus
        self._registry = registry
        self._config_processor = config_processor
        self._local_service_manager = local_service_manager
        self._http_timeout_seconds = http_timeout_seconds

        # Subscribe to events
        self._event_bus.subscribe(ServiceInitialized, self._on_service_initialized, priority=80)
        self._event_bus.subscribe(ServiceConnectionRequested, self._on_connection_requested, priority=100)

        # New: subscribe to reconnection request events
        from mcpstore.core.events.service_events import ReconnectionRequested
        self._event_bus.subscribe(ReconnectionRequested, self._on_reconnection_requested, priority=100)

        logger.info(f"ConnectionManager initialized (bus={hex(id(self._event_bus))}) and subscribed to events")
        logger.debug(f"[CONNECTION] HTTP timeout configured: {self._http_timeout_seconds} seconds")

    async def _on_service_initialized(self, event: ServiceInitialized):
        """
        Handle service initialization completion - trigger connection
        
        ServiceInitialized 表示缓存和生命周期元数据已经写入，此时才发布连接事件。
        """
        logger.info(f"[CONNECTION] Triggering connection for: {event.service_name} (from ServiceInitialized)")

        # Get service configuration（使用异步版本）
        service_config = await self._get_service_config_async(event.agent_id, event.service_name)
        if not service_config:
            logger.error(f"[CONNECTION] No config found for {event.service_name}")
            return

        # Diagnostics: check subscriber count for ServiceConnectionRequested
        try:
            sub_cnt = self._event_bus.get_subscriber_count(ServiceConnectionRequested)
            logger.debug(f"[CONNECTION] Bus {hex(id(self._event_bus))} ServiceConnectionRequested subscribers={sub_cnt}")
        except Exception as e:
            logger.debug(f"[CONNECTION] Subscriber count check failed: {e}")

        # Use configured timeout instead of hardcoded value
        timeout = self._http_timeout_seconds
        logger.debug(f"[CONNECTION] Using configured HTTP timeout: {timeout} seconds for {event.service_name}")

        # Publish connection request event (decoupled)
        connection_request = ServiceConnectionRequested(
            agent_id=event.agent_id,
            service_name=event.service_name,
            service_config=service_config,
            timeout=timeout
        )
        # 异步派发，避免 add_service 阻塞；调用方如需等待可使用 wait_service 等工具
        await self._event_bus.publish(connection_request, wait=False)

    async def _on_connection_requested(self, event: ServiceConnectionRequested):
        """
        Handle connection request - execute actual connection
        """
        logger.info(f"[CONNECTION] Connecting to: {event.service_name} (bus={hex(id(self._event_bus))}, timeout={event.timeout}s)")

        start_time = asyncio.get_event_loop().time()

        try:
            # Determine service type
            if "command" in event.service_config:
                # Local service
                session, tools = await self._connect_local_service(
                    event.service_name, event.service_config, event.timeout
                )
            else:
                # Remote service
                session, tools = await self._connect_remote_service(
                    event.service_name, event.service_config, event.timeout
                )

            connection_time = asyncio.get_event_loop().time() - start_time

            logger.info(
                f"[CONNECTION] Connected: {event.service_name} "
                f"({len(tools)} tools, {connection_time:.2f}s)"
            )

            # Publish connection success event
            connected_event = ServiceConnected(
                agent_id=event.agent_id,
                service_name=event.service_name,
                session=session,
                tools=tools,
                connection_time=connection_time
            )
            # 按调用方指定异步派发；如需一致性由上层等待 Ready/工具事件
            await self._event_bus.publish(connected_event, wait=False)

        except asyncio.TimeoutError:
            elapsed = asyncio.get_event_loop().time() - start_time
            logger.warning(
                f"[CONNECTION] Timeout: {event.service_name} "
                f"(configured={event.timeout}s, elapsed={elapsed:.3f}s)"
            )
            await self._publish_connection_failed(
                event, "Connection timeout", "timeout", 0
            )

        except Exception as e:
            # Demote expected network/connectivity errors to DEGRADED and show friendly message
            network_error = False
            try:
                import httpx  # type: ignore
                if isinstance(e, getattr(httpx, "ConnectError", tuple())) or isinstance(e, getattr(httpx, "ReadTimeout", tuple())):
                    network_error = True
            except Exception:
                pass
            text = str(e)
            if ("all connection attempts failed" in text.lower()) or ("timed out" in text.lower()) or ("certificate" in text.lower()) or ("handshake failure" in text.lower()):
                network_error = True

            # Convert to user-friendly message
            try:
                friendly = ConfigProcessor.get_user_friendly_error(text)
            except Exception:
                friendly = text

            if network_error:
                logger.warning(f"[CONNECTION] Failed: {event.service_name} - {friendly}")
            else:
                logger.error(f"[CONNECTION] Failed: {event.service_name} - {friendly}", exc_info=True)
            await self._publish_connection_failed(
                event, text, "connection_error", 0
            )

    async def _connect_local_service(
        self,
        service_name: str,
        service_config: Dict[str, Any],
        timeout: float
    ) -> Tuple[Any, List[Tuple[str, Dict[str, Any]]]]:
        """Connect to local service"""
        from fastmcp import Client

        # 1. Process configuration
        processed_config = self._config_processor.process_user_config_for_fastmcp({
            "mcpServers": {service_name: service_config}
        })

        # 2. Create client and connect（FastMCP Client 会在 async with 中自动启动本地进程）
        client = Client(processed_config)

        async with asyncio.timeout(timeout):
            async with client:
                tools_list = await client.list_tools()
                processed_tools = self._process_tools(service_name, tools_list)
                return client, processed_tools

    async def _connect_remote_service(
        self,
        service_name: str,
        service_config: Dict[str, Any],
        timeout: float
    ) -> Tuple[Any, List[Tuple[str, Dict[str, Any]]]]:
        """Connect to remote service"""
        from fastmcp import Client

        # 1. Process configuration
        processed_config = self._config_processor.process_user_config_for_fastmcp({
            "mcpServers": {service_name: service_config}
        })

        # 2. Create client and connect
        client = Client(processed_config)

        async with asyncio.timeout(timeout):
            async with client:
                tools_list = await client.list_tools()
                processed_tools = self._process_tools(service_name, tools_list)
                return client, processed_tools

    def _process_tools(
        self,
        service_name: str,
        tools_list: List[Any]
    ) -> List[Tuple[str, Dict[str, Any]]]:
        """Process tool list"""
        processed_tools = []

        for tool in tools_list:
            try:
                original_name = tool.name
                display_name = f"{service_name}_{original_name}"

                # Process parameters
                parameters = {}
                if hasattr(tool, 'inputSchema') and tool.inputSchema:
                    if hasattr(tool.inputSchema, 'model_dump'):
                        parameters = tool.inputSchema.model_dump()
                    elif isinstance(tool.inputSchema, dict):
                        parameters = tool.inputSchema

                # Build tool definition
                tool_def = {
                    "type": "function",
                    "function": {
                        "name": original_name,
                        "display_name": display_name,
                        "description": tool.description if hasattr(tool, 'description') else "",
                        "parameters": parameters,
                        "service_name": service_name
                    }
                }

                processed_tools.append((display_name, tool_def))

            except Exception as e:
                logger.error(f"Failed to process tool {tool.name}: {e}")
                continue

        return processed_tools

    async def _publish_connection_failed(
        self,
        event: ServiceConnectionRequested,
        error_message: str,
        error_type: str,
        retry_count: int
    ):
        """Publish connection failed event"""
        try:
            friendly_message = ConfigProcessor.get_user_friendly_error(error_message or "")
        except Exception:
            friendly_message = error_message
        failed_event = ServiceConnectionFailed(
            agent_id=event.agent_id,
            service_name=event.service_name,
            error_message=friendly_message,
            error_type=error_type,
            retry_count=retry_count
        )
        # 关键修复：使用 wait=True 确保状态更新完成，避免任务被取消导致状态不更新
        await self._event_bus.publish(failed_event, wait=True)

    async def _on_reconnection_requested(self, event: 'ReconnectionRequested'):
        """
        Handle reconnection request - trigger connection again
        """
        logger.info(f"[CONNECTION] Reconnection requested: {event.service_name} (retry={event.retry_count})")

        # Get service configuration（使用异步版本）
        service_config = await self._get_service_config_async(event.agent_id, event.service_name)
        if not service_config:
            logger.error(f"[CONNECTION] No config found for reconnection: {event.service_name}")
            return

        # Publish connection request event (reuse existing connection logic)
        connection_request = ServiceConnectionRequested(
            agent_id=event.agent_id,
            service_name=event.service_name,
            service_config=service_config,
            timeout=5.0  # Use longer timeout for reconnection
        )
        await self._event_bus.publish(connection_request, wait=True)

    async def _get_service_config_async(self, agent_id: str, service_name: str) -> Dict[str, Any]:
        """
        从服务实体中获取服务配置（异步版本）
        
        在新架构中，服务配置存储在服务实体中（service_entity.config），
        不再从 client_config 中获取。
        """
        logger.debug(f"[CONNECTION] Getting config for {agent_id}:{service_name}")

        # 生成服务全局名称
        service_global_name = self._registry._naming.generate_service_global_name(
            service_name, agent_id
        )
        
        # 从 pykv 获取服务实体
        service_entity = await self._registry._cache_service_manager.get_service(
            service_global_name
        )
        
        if service_entity is None:
            raise RuntimeError(
                f"Service entity does not exist: service_name={service_name}, "
                f"agent_id={agent_id}, global_name={service_global_name}"
            )
        
        service_config = service_entity.config
        if not service_config:
            raise RuntimeError(
                f"Service configuration is empty: service_name={service_name}, "
                f"agent_id={agent_id}, global_name={service_global_name}"
            )
        
        logger.debug(f"[CONNECTION] Found config for {service_name}: {list(service_config.keys())}")
        return service_config
