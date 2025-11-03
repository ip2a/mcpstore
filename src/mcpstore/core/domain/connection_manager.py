"""
连接管理器 - 负责实际的服务连接

职责:
1. 监听 ServiceInitialized 事件，触发连接
2. 执行实际的服务连接（本地/远程）
3. 发布 ServiceConnected/ServiceConnectionFailed 事件
"""

import asyncio
import logging
from typing import Dict, Any, Tuple, List

from mcpstore.core.events.event_bus import EventBus
from mcpstore.core.events.service_events import (
    ServiceInitialized, ServiceConnectionRequested,
    ServiceConnected, ServiceConnectionFailed
)
from mcpstore.core.configuration.config_processor import ConfigProcessor

logger = logging.getLogger(__name__)


class ConnectionManager:
    """
    连接管理器

    职责:
    1. 监听 ServiceInitialized 事件，触发连接
    2. 执行实际的服务连接（本地/远程）
    3. 发布 ServiceConnected/ServiceConnectionFailed 事件
    """

    def __init__(
        self,
        event_bus: EventBus,
        registry: 'CoreRegistry',
        config_processor: 'ConfigProcessor',
        local_service_manager: 'LocalServiceManagerAdapter'
    ):
        self._event_bus = event_bus
        self._registry = registry
        self._config_processor = config_processor
        self._local_service_manager = local_service_manager

        # 订阅事件
        self._event_bus.subscribe(ServiceInitialized, self._on_service_initialized, priority=80)
        self._event_bus.subscribe(ServiceConnectionRequested, self._on_connection_requested, priority=100)

        # 🆕 订阅重连请求事件
        from mcpstore.core.events.service_events import ReconnectionRequested
        self._event_bus.subscribe(ReconnectionRequested, self._on_reconnection_requested, priority=100)

        logger.info(f"ConnectionManager initialized (bus={hex(id(self._event_bus))}) and subscribed to events")

    async def _on_service_initialized(self, event: ServiceInitialized):
        """
        处理服务初始化完成 - 触发连接
        """
        logger.info(f"[CONNECTION] Triggering connection for: {event.service_name}")

        # 获取服务配置
        service_config = self._get_service_config(event.agent_id, event.service_name)
        if not service_config:
            logger.error(f"[CONNECTION] No config found for {event.service_name}")
            return

        # Diagnostics: check subscriber count for ServiceConnectionRequested
        try:
            sub_cnt = self._event_bus.get_subscriber_count(ServiceConnectionRequested)
            logger.debug(f"[CONNECTION] Bus {hex(id(self._event_bus))} ServiceConnectionRequested subscribers={sub_cnt}")
        except Exception as e:
            logger.debug(f"[CONNECTION] Subscriber count check failed: {e}")

        # 发布连接请求事件（解耦）
        connection_request = ServiceConnectionRequested(
            agent_id=event.agent_id,
            service_name=event.service_name,
            service_config=service_config,
            timeout=3.0
        )
        # Use synchronous dispatch to avoid event-loop race during restart/initialization
        await self._event_bus.publish(connection_request, wait=True)

    async def _on_connection_requested(self, event: ServiceConnectionRequested):
        """
        处理连接请求 - 执行实际连接
        """
        logger.info(f"[CONNECTION] Connecting to: {event.service_name} (bus={hex(id(self._event_bus))})")

        start_time = asyncio.get_event_loop().time()

        try:
            # 判断服务类型
            if "command" in event.service_config:
                # 本地服务
                session, tools = await self._connect_local_service(
                    event.service_name, event.service_config, event.timeout
                )
            else:
                # 远程服务
                session, tools = await self._connect_remote_service(
                    event.service_name, event.service_config, event.timeout
                )

            connection_time = asyncio.get_event_loop().time() - start_time

            logger.info(
                f"[CONNECTION] Connected: {event.service_name} "
                f"({len(tools)} tools, {connection_time:.2f}s)"
            )

            # 发布连接成功事件
            connected_event = ServiceConnected(
                agent_id=event.agent_id,
                service_name=event.service_name,
                session=session,
                tools=tools,
                connection_time=connection_time
            )
            await self._event_bus.publish(connected_event)

        except asyncio.TimeoutError:
            logger.warning(f"[CONNECTION] Timeout: {event.service_name}")
            await self._publish_connection_failed(
                event, "Connection timeout", "timeout", 0
            )

        except Exception as e:
            # Demote expected network/connectivity errors to WARNING and show friendly message
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
        """连接本地服务"""
        from fastmcp import Client

        # 1. 启动本地进程
        success, message = await self._local_service_manager.start_local_service(
            service_name, service_config
        )
        if not success:
            raise RuntimeError(f"Failed to start local service: {message}")

        # 2. 处理配置
        processed_config = self._config_processor.process_user_config_for_fastmcp({
            "mcpServers": {service_name: service_config}
        })

        # 3. 创建客户端并连接
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
        """连接远程服务"""
        from fastmcp import Client

        # 1. 处理配置
        processed_config = self._config_processor.process_user_config_for_fastmcp({
            "mcpServers": {service_name: service_config}
        })

        # 2. 创建客户端并连接
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
        """处理工具列表"""
        processed_tools = []

        for tool in tools_list:
            try:
                original_name = tool.name
                display_name = f"{service_name}_{original_name}"

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
        """发布连接失败事件"""
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
        await self._event_bus.publish(failed_event)

    async def _on_reconnection_requested(self, event: 'ReconnectionRequested'):
        """
        处理重连请求 - 重新触发连接
        """
        logger.info(f"[CONNECTION] Reconnection requested: {event.service_name} (retry={event.retry_count})")

        # 获取服务配置
        service_config = self._get_service_config(event.agent_id, event.service_name)
        if not service_config:
            logger.error(f"[CONNECTION] No config found for reconnection: {event.service_name}")
            return

        # 发布连接请求事件（复用现有连接逻辑）
        connection_request = ServiceConnectionRequested(
            agent_id=event.agent_id,
            service_name=event.service_name,
            service_config=service_config,
            timeout=5.0  # 重连时使用更长的超时
        )
        await self._event_bus.publish(connection_request, wait=True)

    def _get_service_config(self, agent_id: str, service_name: str) -> Dict[str, Any]:
        """从缓存中获取服务配置"""
        # 通过 client_id 获取配置
        client_id = self._registry.get_service_client_id(agent_id, service_name)
        if not client_id:
            return {}

        client_config = self._registry.get_client_config_from_cache(client_id)
        if not client_config:
            return {}

        mcp_servers = client_config.get("mcpServers", {})
        return mcp_servers.get(service_name, {})

