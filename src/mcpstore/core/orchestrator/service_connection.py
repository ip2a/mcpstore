"""
MCPOrchestrator Service Connection Module
Service connection module - contains service connection and state management
"""

import asyncio
import logging
from typing import Dict, List, Any, Optional, Tuple

from mcpstore.core.configuration.config_processor import ConfigProcessor
from fastmcp import Client
from mcpstore.core.lifecycle import HealthStatus, HealthCheckResult
from mcpstore.core.lifecycle.health_bridge import HealthStatusBridge
from .health_monitoring import HealthMonitoringMixin

logger = logging.getLogger(__name__)

class ServiceConnectionMixin(HealthMonitoringMixin):
    """Service connection mixin class"""

    async def connect_service(self, name: str, service_config: Dict[str, Any] = None, url: str = None, agent_id: str = None) -> Tuple[bool, str]:
        """
        Connect to specified service (supports local and remote services) and update cache

        🔧 缓存优先架构：优先从缓存获取配置，支持完整的服务配置

        Args:
            name: Service name
            service_config: Complete service configuration (preferred, supports all service types)
            url: Service URL (legacy parameter, only for simple HTTP services)
            agent_id: Agent ID (optional, if not provided will use global_agent_store_id)

        Returns:
            Tuple[bool, str]: (success status, message)
        """
        try:
            # 确定Agent ID
            agent_key = agent_id or self.client_manager.global_agent_store_id

            # 🔧 缓存优先：从缓存获取服务配置
            if service_config is None:
                service_config = self.registry.get_service_config_from_cache(agent_key, name)
                if not service_config:
                    return False, f"Service configuration not found in cache for {name}. This indicates a system issue."

            # 如果提供了URL，更新配置（向后兼容）
            if url:
                service_config = service_config.copy()  # 不修改原始缓存
                service_config["url"] = url

            # 判断是本地服务还是远程服务
            if "command" in service_config:
                # 本地服务：先启动进程，再连接
                return await self._connect_local_service(name, service_config, agent_key)
            else:
                # 远程服务：直接连接
                return await self._connect_remote_service(name, service_config, agent_key)

        except Exception as e:
            logger.error(f"Failed to connect service {name}: {e}")
            return False, str(e)

    async def _connect_local_service(self, name: str, service_config: Dict[str, Any], agent_id: str) -> Tuple[bool, str]:
        """连接本地服务并更新缓存"""
        try:
            # 1. 启动本地服务进程
            success, message = await self.local_service_manager.start_local_service(name, service_config)
            if not success:
                return False, f"Failed to start local service: {message}"

            #创建客户端连接
            # 本地服务通常使用 stdio 传输
            local_config = service_config.copy()

            # 🔧 修复：使用 ConfigProcessor 处理配置（与remote service保持一致）
            from mcpstore.core.configuration.config_processor import ConfigProcessor
            processed_config = ConfigProcessor.process_user_config_for_fastmcp({
                "mcpServers": {name: local_config}
            })

            if name not in processed_config.get("mcpServers", {}):
                return False, "Local service configuration processing failed"

            # 创建客户端
            client = Client(processed_config)

            # 尝试连接和获取工具列表
            try:
                async with client:
                    tools = await client.list_tools()

                    # 🔧 修复：更新Registry缓存
                    await self._update_service_cache(agent_id, name, client, tools, service_config)

                    # 更新客户端缓存（保持向后兼容）
                    self.clients[name] = client

                    # 🔧 修复：通知生命周期管理器连接成功
                    await self.lifecycle_manager.handle_health_check_result(
                        agent_id=agent_id,
                        service_name=name,
                        success=True,
                        response_time=0.0,
                        error_message=None
                    )

                    logger.info(f"Local service {name} connected successfully with {len(tools)} tools for agent {agent_id}")
                    return True, f"Local service connected successfully with {len(tools)} tools"
            except Exception as e:
                error_msg = str(e)
                logger.error(f"Failed to connect to local service {name}: {error_msg}")

                # 🔧 修复：清理资源，避免僵尸进程
                try:
                    # 停止本地服务进程
                    await self.local_service_manager.stop_local_service(name)
                    logger.debug(f"Cleaned up local service process for {name}")
                except Exception as cleanup_error:
                    logger.error(f"Failed to cleanup local service {name}: {cleanup_error}")

                # 清理客户端缓存
                if name in self.clients:
                    try:
                        client = self.clients[name]
                        if hasattr(client, 'close'):
                            await client.close()
                        del self.clients[name]
                        logger.debug(f"Cleaned up client cache for {name}")
                    except Exception as cleanup_error:
                        logger.error(f"Failed to cleanup client cache for {name}: {cleanup_error}")

                # 通知生命周期管理器连接失败
                await self.lifecycle_manager.handle_health_check_result(
                    agent_id=agent_id,
                    service_name=name,
                    success=False,
                    response_time=0.0,
                    error_message=error_msg
                )

                return False, f"Failed to connect to local service: {error_msg}"

        except Exception as e:
            error_msg = str(e)
            logger.error(f"Error connecting local service {name}: {error_msg}")

            # 🔧 修复：清理资源，避免僵尸进程
            try:
                # 停止本地服务进程
                await self.local_service_manager.stop_local_service(name)
                logger.debug(f"Cleaned up local service process for {name} after outer exception")
            except Exception as cleanup_error:
                logger.error(f"Failed to cleanup local service {name} after outer exception: {cleanup_error}")

            # 清理客户端缓存
            if name in self.clients:
                try:
                    client = self.clients[name]
                    if hasattr(client, 'close'):
                        await client.close()
                    del self.clients[name]
                    logger.debug(f"Cleaned up client cache for {name} after outer exception")
                except Exception as cleanup_error:
                    logger.error(f"Failed to cleanup client cache for {name} after outer exception: {cleanup_error}")

            # 通知生命周期管理器连接失败
            await self.lifecycle_manager.handle_health_check_result(
                agent_id=agent_id,
                service_name=name,
                success=False,
                response_time=0.0,
                error_message=error_msg
            )

            return False, error_msg

    async def _connect_remote_service(self, name: str, service_config: Dict[str, Any], agent_id: str) -> Tuple[bool, str]:
        """连接远程服务并更新缓存"""
        try:
            # 🔧 修复：使用ConfigProcessor处理配置，确保transport字段正确
            from mcpstore.core.configuration.config_processor import ConfigProcessor

            # 构造配置格式
            user_config = {"mcpServers": {name: service_config}}

            # 使用ConfigProcessor处理配置（与register_json_services保持一致）
            processed_config = ConfigProcessor.process_user_config_for_fastmcp(user_config)

            # 检查处理后的配置
            if name not in processed_config.get("mcpServers", {}):
                return False, f"Service configuration processing failed for {name}"

            # 创建新的客户端（使用处理后的配置）
            client = Client(processed_config)

            # 尝试连接
            try:
                logger.info(f" [REMOTE_SERVICE] 准备进入 async with client 上下文: {name}")
                async with client:
                    logger.info(f" [REMOTE_SERVICE] 成功进入 async with client 上下文: {name}")
                    logger.info(f" [REMOTE_SERVICE] 准备调用 client.list_tools(): {name}")
                    tools = await client.list_tools()
                    logger.info(f" [REMOTE_SERVICE] 成功获取工具列表，数量: {len(tools)}")

                    # 🔧 修复：更新Registry缓存
                    await self._update_service_cache(agent_id, name, client, tools, service_config)

                    # 更新客户端缓存（保持向后兼容）
                    self.clients[name] = client

                    # 🔧 修复：通知生命周期管理器连接成功
                    await self.lifecycle_manager.handle_health_check_result(
                        agent_id=agent_id,
                        service_name=name,
                        success=True,
                        response_time=0.0,
                        error_message=None
                    )

                    logger.info(f"Remote service {name} connected successfully with {len(tools)} tools for agent {agent_id}")
                    return True, f"Remote service connected successfully with {len(tools)} tools"
            except Exception as e:
                error_msg = str(e)
                logger.warning(f"Failed to connect to remote service {name}: {error_msg}")

                # 🔧 修复：清理资源，避免资源泄漏
                # 清理客户端缓存
                if name in self.clients:
                    try:
                        cached_client = self.clients[name]
                        if hasattr(cached_client, 'close'):
                            await cached_client.close()
                        del self.clients[name]
                        logger.debug(f"Cleaned up client cache for remote service {name}")
                    except Exception as cleanup_error:
                        logger.error(f"Failed to cleanup client cache for remote service {name}: {cleanup_error}")

                # 确保当前客户端也被正确关闭
                try:
                    if hasattr(client, 'close'):
                        await client.close()
                    logger.debug(f"Closed current client for remote service {name}")
                except Exception as cleanup_error:
                    logger.warning(f"Failed to close current client for remote service {name}: {cleanup_error}")

                # 通知生命周期管理器连接失败
                await self.lifecycle_manager.handle_health_check_result(
                    agent_id=agent_id,
                    service_name=name,
                    success=False,
                    response_time=0.0,
                    error_message=error_msg
                )

                return False, error_msg

        except Exception as e:
            error_msg = str(e)
            logger.error(f"Error connecting remote service {name}: {error_msg}")

            # 🔧 修复：清理资源，避免资源泄漏
            # 清理客户端缓存
            if name in self.clients:
                try:
                    cached_client = self.clients[name]
                    if hasattr(cached_client, 'close'):
                        await cached_client.close()
                    del self.clients[name]
                    logger.debug(f"Cleaned up client cache for remote service {name} after outer exception")
                except Exception as cleanup_error:
                    logger.error(f"Failed to cleanup client cache for remote service {name} after outer exception: {cleanup_error}")

            # 通知生命周期管理器连接失败
            await self.lifecycle_manager.handle_health_check_result(
                agent_id=agent_id,
                service_name=name,
                success=False,
                response_time=0.0,
                error_message=error_msg
            )

            return False, error_msg

    async def _update_service_cache(self, agent_id: str, service_name: str, client: Client, tools: List[Any], service_config: Dict[str, Any]):
        """
        更新服务缓存（工具定义、映射关系等）

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            client: FastMCP客户端
            tools: 工具列表
            service_config: 服务配置
        """
        try:
            # 🔧 优雅修复：智能清理缓存，保留Agent-Client映射
            existing_session = self.registry.get_session(agent_id, service_name)
            if existing_session:
                # 服务已存在，只清理工具缓存，保留Agent-Client映射
                logger.debug(f"🔧 [CACHE_UPDATE] 服务 {service_name} 已存在，执行智能清理")
                self.registry.clear_service_tools_only(agent_id, service_name)
            else:
                # 新服务，不需要清理任何缓存
                logger.debug(f"🔧 [CACHE_UPDATE] 服务 {service_name} 是新服务，跳过清理")

            # 处理工具定义（复用register_json_services的逻辑）
            processed_tools = []
            for tool in tools:
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

            # 🔧 优雅修复：添加到Registry缓存，保留现有映射关系
            self.registry.add_service(
                agent_id=agent_id,
                name=service_name,
                session=client,
                tools=processed_tools,
                preserve_mappings=True  # 保留现有的Agent-Client映射
            )

            # 标记长连接服务
            if self._is_long_lived_service(service_config):
                self.registry.mark_as_long_lived(agent_id, service_name)

            # 🔧 重要：注册客户端到 Agent 客户端缓存
            client_id = self.registry.get_service_client_id(agent_id, service_name)
            if client_id:
                self.registry.add_agent_client_mapping(agent_id, client_id)
                logger.debug(f"🔧 [CLIENT_REGISTER] 注册客户端 {client_id} 到 Agent {agent_id}")
            else:
                logger.warning(f"🔧 [CLIENT_REGISTER] 无法获取服务 {service_name} 的 Client ID")

            # 通知生命周期管理器连接成功
            await self.lifecycle_manager.handle_health_check_result(
                agent_id=agent_id,
                service_name=service_name,
                success=True,
                response_time=0.0,  # 连接时间，可以后续优化
                error_message=None
            )

            # 将服务加入内容监控（用于运行期工具变化的兜底刷新）
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

    async def disconnect_service(self, url_or_name: str) -> bool:
        """从配置中移除服务并更新global_agent_store"""
        logger.info(f"Removing service: {url_or_name}")

        # 查找要移除的服务名
        name_to_remove = None
        for name, server in self.global_agent_store_config.get("mcpServers", {}).items():
            if name == url_or_name or server.get("url") == url_or_name:
                name_to_remove = name
                break

        if name_to_remove:
            # 从global_agent_store_config中移除
            if name_to_remove in self.global_agent_store_config["mcpServers"]:
                del self.global_agent_store_config["mcpServers"][name_to_remove]

            # 从配置文件中移除
            ok = self.mcp_config.remove_service(name_to_remove)
            if not ok:
                logger.warning(f"Failed to remove service {name_to_remove} from configuration file")

            # 从registry中移除
            self.registry.remove_service(name_to_remove)

            # 重新创建global_agent_store
            if self.global_agent_store_config.get("mcpServers"):
                self.global_agent_store = Client(self.global_agent_store_config)

                # 更新所有agent_clients
                for agent_id in list(self.agent_clients.keys()):
                    self.agent_clients[agent_id] = Client(self.global_agent_store_config)
                    logger.info(f"Updated client for agent {agent_id} after removing service")

            else:
                # 如果没有服务了，清除global_agent_store
                self.global_agent_store = None
                # 清除所有agent_clients
                self.agent_clients.clear()

            return True
        else:
            logger.warning(f"Service {url_or_name} not found in configuration.")
            return False

    async def refresh_services(self):
        """手动刷新所有服务连接（重新加载mcp.json）"""
        # 🔧 修复：使用统一同步管理器进行同步
        if hasattr(self, 'sync_manager') and self.sync_manager:
            await self.sync_manager.sync_global_agent_store_from_mcp_json()
        else:
            logger.warning("Sync manager not available, cannot refresh services")

    async def refresh_service_content(self, service_name: str, agent_id: str = None) -> bool:
        """手动刷新指定服务的内容（工具、资源、提示词）"""
        agent_key = agent_id or self.client_manager.global_agent_store_id
        return await self.content_manager.force_update_service_content(agent_key, service_name)

    async def is_service_healthy(self, name: str, client_id: Optional[str] = None) -> bool:
        """
        检查服务是否健康（增强版本，支持分级健康状态和智能超时）

        Args:
            name: 服务名
            client_id: 可选的客户端ID，用于多客户端环境

        Returns:
            bool: 服务是否健康（True表示healthy/warning/slow，False表示unhealthy）
        """
        result = await self.check_service_health_detailed(name, client_id)
        # 🆕 使用统一的健康状态判断逻辑
        return HealthStatusBridge.is_health_status_positive(result.status)

    def _normalize_service_config(self, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """规范化服务配置，确保包含必要的字段"""
        if not service_config:
            return service_config

        # 创建配置副本
        normalized = service_config.copy()

        # 自动推断transport类型（如果未指定）
        if "url" in normalized and "transport" not in normalized:
            url = normalized["url"]
            if "/sse" in url.lower():
                normalized["transport"] = "sse"
            else:
                normalized["transport"] = "streamable-http"
            logger.debug(f"Auto-inferred transport type: {normalized['transport']} for URL: {url}")

        return normalized
