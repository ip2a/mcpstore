"""
MCPOrchestrator Service Connection Module
Service connection module - contains service connection and state management
"""

import logging
from typing import Dict, List, Any, Optional, Tuple

from fastmcp import Client

from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)

class ServiceConnectionMixin:
    """Service connection mixin class"""

    async def connect_service(self, name: str, service_config: Dict[str, Any] = None, url: str = None, agent_id: str = None) -> Tuple[bool, str]:
        """
        Connect to specified service (supports local and remote services) and update cache

         ??????????????????????????

        Args:
            name: Service name
            service_config: Complete service configuration (preferred, supports all service types)
            url: Service URL (legacy parameter, only for simple HTTP services)
            agent_id: Agent ID (optional, if not provided will use global_agent_store_id)

        Returns:
            Tuple[bool, str]: (success status, message)
        """
        try:
            # ??Agent ID
            agent_key = agent_id or self.client_manager.global_agent_store_id

            #  ??????????????
            if service_config is None:
                service_config = self.registry.get_service_config_from_cache(agent_key, name)
                if not service_config:
                    return False, f"Service configuration not found in cache for {name}. This indicates a system issue."

            # ?????URL???????????
            if url:
                service_config = service_config.copy()  # ???????
                service_config["url"] = url

            # ?????????????
            if "command" in service_config:
                # ??????????????
                return await self._connect_local_service(name, service_config, agent_key)
            else:
                # ?????????
                return await self._connect_remote_service(name, service_config, agent_key)

        except Exception as e:
            logger.error(f"Failed to connect service {name}: {e}")
            return False, str(e)

    async def _connect_local_service(self, name: str, service_config: Dict[str, Any], agent_id: str) -> Tuple[bool, str]:
        """???????????"""
        try:
            # 1. ????????
            success, message = await self.local_service_manager.start_local_service(name, service_config)
            if not success:
                return False, f"Failed to start local service: {message}"

            #???????
            # ???????? stdio ??
            local_config = service_config.copy()

            #  ????? ConfigProcessor ??????remote service?????
            from mcpstore.core.configuration.config_processor import ConfigProcessor
            processed_config = ConfigProcessor.process_user_config_for_fastmcp({
                "mcpServers": {name: local_config}
            })

            if name not in processed_config.get("mcpServers", {}):
                return False, "Local service configuration processing failed"

            # ?????
            client = Client(processed_config)

            # ???????????
            try:
                async with client:
                    tools = await client.list_tools()

                    #  ?????Registry????????client?Registry?????????
                    await self._update_service_cache(agent_id, name, client, tools, service_config)

                    # ?????? client?async with ????????
                    # self.clients[name] = client

                    #  发布连接成功事件
                    from mcpstore.core.events.service_events import ServiceConnected
                    # 构建完整的工具定义，包含 description 和 inputSchema
                    tools_with_full_def = []
                    for tool in tools:
                        tool_def = {
                            "description": getattr(tool, 'description', ''),
                            "inputSchema": tool.inputSchema if hasattr(tool, 'inputSchema') else {}
                        }
                        tools_with_full_def.append((tool.name, tool_def))
                    
                    connection_event = ServiceConnected(
                        agent_id=agent_id,
                        service_name=name,
                        session=client,
                        tools=tools_with_full_def,
                        connection_time=0.0
                    )
                    await self.container._event_bus.publish(connection_event)

                    logger.info(f"Local service {name} connected successfully with {len(tools)} tools for agent {agent_id}")
                    return True, f"Local service connected successfully with {len(tools)} tools"
            except Exception as e:
                error_msg = str(e)
                logger.error(f"Failed to connect to local service {name}: {error_msg}")

                #  ??????????????
                try:
                    # ????????
                    await self.local_service_manager.stop_local_service(name)
                    logger.debug(f"Cleaned up local service process for {name}")
                except Exception as cleanup_error:
                    logger.error(f"Failed to cleanup local service {name}: {cleanup_error}")

                # ???????
                if name in self.clients:
                    try:
                        client = self.clients[name]
                        if hasattr(client, 'close'):
                            await client.close()
                        del self.clients[name]
                        logger.debug(f"Cleaned up client cache for {name}")
                    except Exception as cleanup_error:
                        logger.error(f"Failed to cleanup client cache for {name}: {cleanup_error}")

                # 发布连接失败事件
                from mcpstore.core.events.service_events import ServiceConnectionFailed
                logger.info(f"[SERVICE_CONNECTION] Publishing ServiceConnectionFailed event for {name}: {error_msg}")
                connection_failed_event = ServiceConnectionFailed(
                    agent_id=agent_id,
                    service_name=name,
                    error_message=error_msg,
                    error_type="connection",
                    retry_count=0
                )
                await self.container._event_bus.publish(connection_failed_event)
                logger.info(f"[SERVICE_CONNECTION] ServiceConnectionFailed event published for {name}")

                return False, f"Failed to connect to local service: {error_msg}"

        except Exception as e:
            error_msg = str(e)
            logger.error(f"Error connecting local service {name}: {error_msg}")

            #  ??????????????
            try:
                # ????????
                await self.local_service_manager.stop_local_service(name)
                logger.debug(f"Cleaned up local service process for {name} after outer exception")
            except Exception as cleanup_error:
                logger.error(f"Failed to cleanup local service {name} after outer exception: {cleanup_error}")

            # ???????
            if name in self.clients:
                try:
                    client = self.clients[name]
                    if hasattr(client, 'close'):
                        await client.close()
                    del self.clients[name]
                    logger.debug(f"Cleaned up client cache for {name} after outer exception")
                except Exception as cleanup_error:
                    logger.error(f"Failed to cleanup client cache for {name} after outer exception: {cleanup_error}")

            # ?????????????
            await self.lifecycle_manager.handle_health_check_result(
                agent_id=agent_id,
                service_name=name,
                success=False,
                response_time=0.0,
                error_message=error_msg
            )

            return False, error_msg

    async def _connect_remote_service(self, name: str, service_config: Dict[str, Any], agent_id: str) -> Tuple[bool, str]:
        """???????????"""
        try:
            #  ?????ConfigProcessor???????transport????
            from mcpstore.core.configuration.config_processor import ConfigProcessor

            # ??????
            user_config = {"mcpServers": {name: service_config}}

            # ??ConfigProcessor??????register_json_services?????
            processed_config = ConfigProcessor.process_user_config_for_fastmcp(user_config)

            # ????????
            if name not in processed_config.get("mcpServers", {}):
                return False, f"Service configuration processing failed for {name}"

            # ?????????????????
            client = Client(processed_config)

            # ????
            try:
                logger.info(f" [REMOTE_SERVICE] ???? async with client ???: {name}")
                async with client:
                    logger.info(f" [REMOTE_SERVICE] ???? async with client ???: {name}")
                    logger.info(f" [REMOTE_SERVICE] ???? client.list_tools(): {name}")
                    tools = await client.list_tools()
                    logger.info(f" [REMOTE_SERVICE] ???????????: {len(tools)}")

                    #  ?????Registry??
                    await self._update_service_cache(agent_id, name, client, tools, service_config)

                    # ????? client?async with ?????????
                    # self.clients[name] = client

                    # 发布连接成功事件
                    from mcpstore.core.events.service_events import ServiceConnected
                    # 构建完整的工具定义，包含 description 和 inputSchema
                    tools_with_full_def = []
                    for tool in tools:
                        tool_def = {
                            "description": getattr(tool, 'description', ''),
                            "inputSchema": tool.inputSchema if hasattr(tool, 'inputSchema') else {}
                        }
                        tools_with_full_def.append((tool.name, tool_def))
                    
                    connection_event = ServiceConnected(
                        agent_id=agent_id,
                        service_name=name,
                        session=client,
                        tools=tools_with_full_def,
                        connection_time=0.0
                    )
                    await self.container._event_bus.publish(connection_event)

                    logger.info(f"Remote service {name} connected successfully with {len(tools)} tools for agent {agent_id}")
                    return True, f"Remote service connected successfully with {len(tools)} tools"
            except Exception as e:
                error_msg = str(e)
                logger.warning(f"Failed to connect to remote service {name}: {error_msg}")

                #  ??????????????
                # ???????
                if name in self.clients:
                    try:
                        cached_client = self.clients[name]
                        if hasattr(cached_client, 'close'):
                            await cached_client.close()
                        del self.clients[name]
                        logger.debug(f"Cleaned up client cache for remote service {name}")
                    except Exception as cleanup_error:
                        logger.error(f"Failed to cleanup client cache for remote service {name}: {cleanup_error}")

                # ?????????????
                try:
                    if hasattr(client, 'close'):
                        await client.close()
                    logger.debug(f"Closed current client for remote service {name}")
                except Exception as cleanup_error:
                    logger.warning(f"Failed to close current client for remote service {name}: {cleanup_error}")

                # ????????????????????
                failure_reason = None
                try:
                    status_code = getattr(getattr(e, 'response', None), 'status_code', None)
                    if status_code in (401, 403):
                        failure_reason = 'auth_failed'
                    else:
                        lower_msg = error_msg.lower()
                        if any(word in lower_msg for word in ['unauthorized', 'forbidden', 'invalid token', 'invalid api key']):
                            failure_reason = 'auth_failed'
                except Exception:
                    pass
                try:
                    metadata = await self.registry._service_state_service.get_service_metadata_async(agent_id, name)
                    if metadata:
                        metadata.failure_reason = failure_reason
                        metadata.error_message = error_msg
                        self.registry.set_service_metadata(agent_id, name, metadata)
                except Exception as e:
                    logger.error(f"Failed to update metadata for {name}: {e}")
                    raise

                # 发布连接失败事件
                from mcpstore.core.events.service_events import ServiceConnectionFailed
                connection_failed_event = ServiceConnectionFailed(
                    agent_id=agent_id,
                    service_name=name,
                    error_message=error_msg,
                    error_type=failure_reason or "connection",
                    retry_count=0
                )
                await self.container._event_bus.publish(connection_failed_event)

                return False, error_msg

        except Exception as e:
            error_msg = str(e)
            logger.error(f"Error connecting remote service {name}: {error_msg}")

            #  ??????????????
            # ???????
            if name in self.clients:
                try:
                    cached_client = self.clients[name]
                    if hasattr(cached_client, 'close'):
                        await cached_client.close()
                    del self.clients[name]
                    logger.debug(f"Cleaned up client cache for remote service {name} after outer exception")
                except Exception as cleanup_error:
                    logger.error(f"Failed to cleanup client cache for remote service {name} after outer exception: {cleanup_error}")

            # ??????????????????????????
            failure_reason = None
            try:
                status_code = getattr(getattr(e, 'response', None), 'status_code', None)
                if status_code in (401, 403):
                    failure_reason = 'auth_failed'
                else:
                    lower_msg = error_msg.lower()
                    if any(word in lower_msg for word in ['unauthorized', 'forbidden', 'invalid token', 'invalid api key']):
                        failure_reason = 'auth_failed'
            except Exception:
                pass
            try:
                metadata = await self.registry._service_state_service.get_service_metadata_async(agent_id, name)
                if metadata:
                    metadata.failure_reason = failure_reason
                    metadata.error_message = error_msg
                    self.registry.set_service_metadata(agent_id, name, metadata)
            except Exception as e:
                logger.error(f"Failed to update metadata for {name}: {e}")
                raise

            # 发布连接失败事件
            from mcpstore.core.events.service_events import ServiceConnectionFailed
            logger.info(f"[SERVICE_CONNECTION] Publishing ServiceConnectionFailed event for {name}: {error_msg}")
            connection_failed_event = ServiceConnectionFailed(
                agent_id=agent_id,
                service_name=name,
                error_message=error_msg,
                error_type="connection",
                retry_count=0
            )
            await self.container._event_bus.publish(connection_failed_event)
            logger.info(f"[SERVICE_CONNECTION] ServiceConnectionFailed event published for {name}")

            return False, error_msg

    async def _update_service_cache(self, agent_id: str, service_name: str, client: Client, tools: List[Any], service_config: Dict[str, Any]):
        """
        ??????????????????

        Args:
            agent_id: Agent ID
            service_name: ????
            client: FastMCP???
            tools: ????
            service_config: ????
        """
        try:
            # ?????????register_json_services????
            processed_tools = []
            for tool in tools:
                try:
                    original_tool_name = tool.name
                    display_name = self._generate_display_name(original_tool_name, service_name)

                    # ????
                    parameters = {}
                    if hasattr(tool, 'inputSchema') and tool.inputSchema:
                        if hasattr(tool.inputSchema, 'model_dump'):
                            parameters = tool.inputSchema.model_dump()
                        elif isinstance(tool.inputSchema, dict):
                            parameters = tool.inputSchema

                    # ??????
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

            # 使用 per-agent 锁保证并发安全，使用异步版本避免事件循环冲突
            locks = getattr(self, 'store', None)
            agent_locks = getattr(locks, 'agent_locks', None) if locks else None
            if agent_locks is None:
                logger.warning("AgentLocks not available; proceeding without per-agent lock for cache update")
                # 检查是否已有会话
                existing_session = self.registry.get_session(agent_id, service_name)
                if existing_session:
                    logger.debug(f" [CACHE_UPDATE] 服务 {service_name} 已存在，清理工具缓存")
                    self.registry.clear_service_tools_only(agent_id, service_name)
                else:
                    logger.debug(f" [CACHE_UPDATE] 服务 {service_name} 不存在，新建")

                # Use a stable per-service session handle (not a live client)
                session_handle = existing_session if existing_session is not None else object()
                # 使用异步版本避免事件循环冲突
                await self.registry.add_service_async(
                    agent_id=agent_id,
                    name=service_name,
                    session=session_handle,
                    tools=processed_tools,
                    service_config=service_config,
                    state=ServiceConnectionState.HEALTHY,
                    preserve_mappings=True
                )

                if self._is_long_lived_service(service_config):
                    self.registry.mark_as_long_lived(agent_id, service_name)

                # 获取或创建 client_id 映射（使用异步版本）
                client_id = await self.registry.get_service_client_id_async(agent_id, service_name)
                if not client_id:
                    # 为远程服务生成新的 client_id
                    import uuid
                    client_id = f"remote_{service_name}_{uuid.uuid4().hex[:8]}"
                    # 通过 CacheLayerManager 存储 service-client 映射
                    await self.registry._cache_layer_manager.put_relation(
                        "service_client",
                        f"{agent_id}:{service_name}",
                        {"agent_id": agent_id, "service_name": service_name, "client_id": client_id}
                    )
                    logger.info(f" [CLIENT_REGISTER] 为服务 {service_name} 创建新的 Client ID: {client_id}")
                else:
                    logger.debug(f" [CLIENT_REGISTER] 已找到客户端 {client_id} 用于服务 {service_name}")
            else:
                async with agent_locks.write(agent_id):
                    # 检查是否已有会话，保留 Agent-Client 映射
                    existing_session = self.registry.get_session(agent_id, service_name)
                    if existing_session:
                        logger.debug(f" [CACHE_UPDATE] 服务 {service_name} 已存在，清理工具缓存")
                        self.registry.clear_service_tools_only(agent_id, service_name)
                    else:
                        logger.debug(f" [CACHE_UPDATE] 服务 {service_name} 不存在，新建")

                    # 使用 Registry 异步版本避免事件循环冲突
                    session_handle = existing_session if existing_session is not None else object()
                    await self.registry.add_service_async(
                        agent_id=agent_id,
                        name=service_name,
                        session=session_handle,
                        tools=processed_tools,
                        service_config=service_config,
                        state=ServiceConnectionState.HEALTHY,
                        preserve_mappings=True
                    )

                    # 标记长连接服务
                    if self._is_long_lived_service(service_config):
                        self.registry.mark_as_long_lived(agent_id, service_name)

                    # 获取或创建 client_id 映射（使用异步版本）
                    client_id = await self.registry.get_service_client_id_async(agent_id, service_name)
                    if not client_id:
                        # 为远程服务生成新的 client_id
                        import uuid
                        client_id = f"remote_{service_name}_{uuid.uuid4().hex[:8]}"
                        # 通过 CacheLayerManager 存储 service-client 映射
                        await self.registry._cache_layer_manager.put_relation(
                            "service_client",
                            f"{agent_id}:{service_name}",
                            {"agent_id": agent_id, "service_name": service_name, "client_id": client_id}
                        )
                        logger.info(f" [CLIENT_REGISTER] 为服务 {service_name} 创建新的 Client ID: {client_id}")
                    else:
                        logger.debug(f" [CLIENT_REGISTER] 已找到客户端 {client_id} 用于服务 {service_name}")

            # 触发健康检查结果处理
            await self.lifecycle_manager.handle_health_check_result(
                agent_id=agent_id,
                service_name=service_name,
                success=True,
                response_time=0.0,  # ???????????
                error_message=None
            )

            # ?????????????????????????
            try:
                if hasattr(self, 'content_manager') and self.content_manager:
                    self.content_manager.add_service_for_monitoring(agent_id, service_name)
                    logger.debug(f"Added service '{service_name}' (agent '{agent_id}') to content monitoring")
            except Exception as e:
                logger.warning(f"Failed to add service '{service_name}' to content monitoring: {e}")

            logger.info(f"Updated cache for service '{service_name}' with {len(processed_tools)} tools for agent '{agent_id}'")

        except Exception as e:
            logger.error(f"Failed to update service cache for '{service_name}': {e}")

    def _is_long_lived_service(self, service_config: Dict[str, Any]) -> bool:
        """
        ??????????

        Args:
            service_config: ????

        Returns:
            ????????
        """
        # STDIO?????????keep_alive=True?
        if "command" in service_config:
            return service_config.get("keep_alive", True)

        # HTTP?????????
        if "url" in service_config:
            return True

        return False

    def _generate_display_name(self, original_tool_name: str, service_name: str) -> str:
        """
        ?????????????

        Args:
            original_tool_name: ??????
            service_name: ????

        Returns:
            ?????????
        """
        try:
            from mcpstore.core.registry.tool_resolver import ToolNameResolver
            resolver = ToolNameResolver()
            return resolver.create_user_friendly_name(service_name, original_tool_name)
        except Exception as e:
            logger.warning(f"Failed to generate display name for {original_tool_name}: {e}")
            # ???????
            return f"{service_name}_{original_tool_name}"

    async def disconnect_service(self, url_or_name: str) -> bool:
        """???????????global_agent_store"""
        logger.info(f"Removing service: {url_or_name}")

        # ?????????
        name_to_remove = None
        for name, server in self.global_agent_store_config.get("mcpServers", {}).items():
            if name == url_or_name or server.get("url") == url_or_name:
                name_to_remove = name
                break

        if name_to_remove:
            # ?global_agent_store_config???
            if name_to_remove in self.global_agent_store_config["mcpServers"]:
                del self.global_agent_store_config["mcpServers"][name_to_remove]

            # ????????
            ok = self.mcp_config.remove_service(name_to_remove)
            if not ok:
                logger.warning(f"Failed to remove service {name_to_remove} from configuration file")

            # ?registry???（使用异步版本）
            agent_id = self.client_manager.global_agent_store_id
            await self.registry.remove_service_async(agent_id, name_to_remove)

            # ????global_agent_store
            if self.global_agent_store_config.get("mcpServers"):
                self.global_agent_store = Client(self.global_agent_store_config)

                # ????agent_clients
                for agent_id in list(self.agent_clients.keys()):
                    self.agent_clients[agent_id] = Client(self.global_agent_store_config)
                    logger.info(f"Updated client for agent {agent_id} after removing service")

            else:
                # ??????????global_agent_store
                self.global_agent_store = None
                # ????agent_clients
                self.agent_clients.clear()

            return True
        else:
            logger.warning(f"Service {url_or_name} not found in configuration.")
            return False

    async def refresh_services(self):
        """???????????????mcp.json?"""
        #  ????????????????
        if hasattr(self, 'sync_manager') and self.sync_manager:
            await self.sync_manager.sync_global_agent_store_from_mcp_json()
        else:
            logger.warning("Sync manager not available, cannot refresh services")

    async def refresh_service_content(self, service_name: str, agent_id: str = None) -> bool:
        """刷新服务内容（工具、资源等）"""
        if self.content_manager is None:
            raise RuntimeError(
                f"content_manager 未初始化，无法刷新服务内容: service_name={service_name}"
            )
        agent_key = agent_id or self.client_manager.global_agent_store_id
        return await self.content_manager.force_update_service_content(agent_key, service_name)

    async def is_service_healthy(self, name: str, client_id: Optional[str] = None) -> bool:
        """
        ??????????????????

        Args:
            name: ???
            client_id: ??????ID?????????

        Returns:
            bool: True ?????? HEALTHY/WARNING ??
        """
        agent_key = client_id or self.client_manager.global_agent_store_id
        state = self.registry._service_state_service.get_service_state(agent_key, name)
        return state in (ServiceConnectionState.HEALTHY, ServiceConnectionState.WARNING)

    def _normalize_service_config(self, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """?????????????????"""
        if not service_config:
            return service_config

        # ??????
        normalized = service_config.copy()

        # ????transport?????????
        if "url" in normalized and "transport" not in normalized:
            url = normalized["url"]
            if "/sse" in url.lower():
                normalized["transport"] = "sse"
            else:
                normalized["transport"] = "streamable-http"
            logger.debug(f"Auto-inferred transport type: {normalized['transport']} for URL: {url}")

        return normalized
