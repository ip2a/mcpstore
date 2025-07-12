import os, sys
sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

"""
MCP服务编排器

该模块提供了MCPOrchestrator类，用于管理MCP服务的连接、工具调用和查询处理。
它是FastAPI应用程序的核心组件，负责协调客户端和服务之间的交互。
"""

import asyncio
import logging
from typing import Dict, List, Any, Optional, Tuple, Set, Union, AsyncGenerator
from datetime import datetime, timedelta
from urllib.parse import urljoin

from mcpstore.core.registry import ServiceRegistry
from mcpstore.core.client_manager import ClientManager
from mcpstore.core.config_processor import ConfigProcessor
from mcpstore.core.local_service_manager import get_local_service_manager
from fastmcp import Client
from fastmcp.client.transports import (
    MCPConfigTransport,
    StreamableHttpTransport,
    SSETransport,
    PythonStdioTransport,
    NodeStdioTransport,
    UvxStdioTransport,
    NpxStdioTransport
)
from mcpstore.config.json_config import MCPConfig
from mcpstore.core.models.service import TransportType
from mcpstore.core.session_manager import SessionManager
from mcpstore.core.smart_reconnection import SmartReconnectionManager, ReconnectionPriority

logger = logging.getLogger(__name__)

class MCPOrchestrator:
    """
    MCP服务编排器

    负责管理服务连接、工具调用和查询处理。
    """

    def __init__(self, config: Dict[str, Any], registry: ServiceRegistry):
        """
        初始化MCP编排器

        Args:
            config: 配置字典
            registry: 服务注册表实例
        """
        self.config = config
        self.registry = registry
        self.clients: Dict[str, Client] = {}  # key为mcpServers的服务名
        self.main_client: Optional[Client] = None
        self.main_client_ctx = None  # async context manager for main_client
        self.main_config = {"mcpServers": {}}  # 中央配置
        self.agent_clients: Dict[str, Client] = {}  # agent_id -> client映射
        # 使用智能重连管理器替代简单的set
        self.smart_reconnection = SmartReconnectionManager()
        self.react_agent = None

        # 从配置中获取心跳和重连设置
        timing_config = config.get("timing", {})
        self.heartbeat_interval = timedelta(seconds=int(timing_config.get("heartbeat_interval_seconds", 60)))
        self.heartbeat_timeout = timedelta(seconds=int(timing_config.get("heartbeat_timeout_seconds", 180)))
        self.reconnection_interval = timedelta(seconds=int(timing_config.get("reconnection_interval_seconds", 60)))
        self.http_timeout = int(timing_config.get("http_timeout_seconds", 10))

        # 监控任务
        self.heartbeat_task = None
        self.reconnection_task = None
        self.cleanup_task = None
        self.mcp_config = MCPConfig()

        # 资源管理配置
        self.max_reconnection_queue_size = 50  # 最大重连队列大小
        self.cleanup_interval = timedelta(hours=1)  # 清理间隔：1小时
        self.max_heartbeat_history_hours = 24  # 心跳历史保留时间：24小时

        # 客户端管理器
        self.client_manager = ClientManager()

        # 会话管理器
        self.session_manager = SessionManager()

        # 本地服务管理器
        self.local_service_manager = get_local_service_manager()

    async def setup(self):
        """初始化编排器资源（不再做服务注册）"""
        logger.info("Setting up MCP Orchestrator...")
        # 只做必要的资源初始化
        logger.info("MCP Orchestrator setup completed")

    async def cleanup(self):
        """清理编排器资源"""
        logger.info("Cleaning up MCP Orchestrator...")

        # 清理本地服务
        if hasattr(self, 'local_service_manager'):
            await self.local_service_manager.cleanup()

        # 关闭所有客户端连接
        for name, client in self.clients.items():
            try:
                await client.close()
                logger.debug(f"Closed client connection for {name}")
            except Exception as e:
                logger.warning(f"Error closing client {name}: {e}")

        self.clients.clear()
        logger.info("MCP Orchestrator cleanup completed")

    async def start_monitoring(self):
        """启动后台健康检查、重连监视器和资源清理任务（带极端场景处理）"""
        try:
            # 验证配置完整性
            if not self._validate_configuration():
                logger.error("Configuration validation failed, monitoring disabled")
                return False

            logger.info("Starting monitoring tasks...")

            # 启动心跳监视器
            if self.heartbeat_task is None or self.heartbeat_task.done():
                logger.info(f"Starting heartbeat monitor. Interval: {self.heartbeat_interval.total_seconds()}s")
                self.heartbeat_task = asyncio.create_task(self._heartbeat_loop_with_error_handling())

            # 启动重连监视器
            if self.reconnection_task is None or self.reconnection_task.done():
                logger.info(f"Starting reconnection monitor. Interval: {self.reconnection_interval.total_seconds()}s")
                self.reconnection_task = asyncio.create_task(self._reconnection_loop_with_error_handling())

            # 启动资源清理任务
            if self.cleanup_task is None or self.cleanup_task.done():
                logger.info(f"Starting resource cleanup task. Interval: {self.cleanup_interval.total_seconds()}s")
                self.cleanup_task = asyncio.create_task(self._cleanup_loop_with_error_handling())

            return True

        except Exception as e:
            logger.error(f"Failed to start monitoring: {e}")
            # 不抛出异常，允许系统继续运行
            return False

    async def _heartbeat_loop(self):
        """后台循环，用于定期健康检查"""
        while True:
            await asyncio.sleep(self.heartbeat_interval.total_seconds())
            await self._check_services_health()

    async def _check_services_health(self):
        """并发检查所有服务的健康状态"""
        logger.debug("Running concurrent periodic health check for all services...")

        # 收集所有需要检查的服务
        health_check_tasks = []
        for client_id, services in self.registry.sessions.items():
            for name in services:
                task = asyncio.create_task(
                    self._check_single_service_health(name, client_id),
                    name=f"health_check_{name}_{client_id}"
                )
                health_check_tasks.append(task)

        if not health_check_tasks:
            logger.debug("No services to check")
            return

        logger.debug(f"Starting concurrent health check for {len(health_check_tasks)} services")

        try:
            # 并发执行所有健康检查，设置总体超时时间
            results = await asyncio.wait_for(
                asyncio.gather(*health_check_tasks, return_exceptions=True),
                timeout=30.0  # 30秒总体超时
            )

            # 处理结果
            success_count = 0
            failed_count = 0
            for i, result in enumerate(results):
                if isinstance(result, Exception):
                    failed_count += 1
                    logger.warning(f"Health check task failed: {result}")
                elif result:
                    success_count += 1
                else:
                    failed_count += 1

            logger.info(f"Health check completed: {success_count} healthy, {failed_count} failed")

        except asyncio.TimeoutError:
            logger.warning("Health check batch timeout (30s), cancelling remaining tasks")
            # 取消未完成的任务
            for task in health_check_tasks:
                if not task.done():
                    task.cancel()
        except Exception as e:
            logger.error(f"Unexpected error during health check: {e}")

    async def _check_single_service_health(self, name: str, client_id: str) -> bool:
        """检查单个服务的健康状态"""
        try:
            is_healthy = await self.is_service_healthy(name, client_id)
            service_key = f"{client_id}:{name}"

            if is_healthy:
                logger.debug(f"Health check SUCCESS for: {name} (client_id={client_id})")
                self.registry.update_service_health(client_id, name)
                # 如果服务恢复健康，从智能重连队列中移除
                self.smart_reconnection.mark_success(service_key)
                return True
            else:
                logger.warning(f"Health check FAILED for {name} (client_id={client_id})")
                # 推断服务优先级并添加到智能重连队列
                priority = self.smart_reconnection._infer_service_priority(name)
                self.smart_reconnection.add_service(client_id, name, priority)
                return False
        except Exception as e:
            logger.warning(f"Health check error for {name} (client_id={client_id}): {e}")
            # 推断服务优先级并添加到智能重连队列
            priority = self.smart_reconnection._infer_service_priority(name)
            self.smart_reconnection.add_service(client_id, name, priority)
            return False

    async def _reconnection_loop(self):
        """定期尝试重新连接服务的后台循环"""
        while True:
            await asyncio.sleep(self.reconnection_interval.total_seconds())
            await self._attempt_reconnections()

    async def _attempt_reconnections(self):
        """尝试重新连接所有待重连的服务（智能重连策略）"""
        # 获取准备重试的服务列表（按优先级排序）
        ready_services = self.smart_reconnection.get_services_ready_for_retry()

        if not ready_services:
            logger.debug("No services ready for reconnection")
            return

        logger.info(f"Attempting to reconnect {len(ready_services)} service(s) with smart strategy")

        # 清理无效的客户端条目
        valid_client_ids = set(self.client_manager.get_all_clients().keys())
        cleaned_count = self.smart_reconnection.cleanup_invalid_clients(valid_client_ids)
        if cleaned_count > 0:
            logger.info(f"Cleaned up {cleaned_count} invalid client entries from reconnection queue")

        # 按优先级尝试重连
        for entry in ready_services:
            try:
                # 检查client是否仍然有效
                if not self.client_manager.has_client(entry.client_id):
                    logger.info(f"Client {entry.client_id} no longer exists, removing {entry.service_name} from reconnection queue")
                    self.smart_reconnection.remove_service(entry.service_key)
                    continue

                # 尝试重新连接
                logger.debug(f"Attempting reconnection for {entry.service_name} (priority: {entry.priority.name}, "
                           f"failures: {entry.failure_count})")

                success, message = await self.connect_service(entry.service_name)
                if success:
                    logger.info(f"Smart reconnection successful for: {entry.service_name} "
                              f"(priority: {entry.priority.name}, after {entry.failure_count} failures)")
                    self.smart_reconnection.mark_success(entry.service_key)
                else:
                    logger.debug(f"Smart reconnection attempt failed for {entry.service_name}: {message}")
                    self.smart_reconnection.mark_failure(entry.service_key)

            except Exception as e:
                logger.warning(f"Smart reconnection attempt failed for {entry.service_key}: {e}")
                self.smart_reconnection.mark_failure(entry.service_key)

    async def _cleanup_loop(self):
        """定期资源清理循环"""
        while True:
            await asyncio.sleep(self.cleanup_interval.total_seconds())
            await self._perform_cleanup()

    async def _perform_cleanup(self):
        """执行资源清理"""
        logger.debug("Performing periodic resource cleanup...")

        try:
            # 清理过期的心跳记录
            cutoff_time = datetime.now() - timedelta(hours=self.max_heartbeat_history_hours)
            cleaned_services = 0
            cleaned_agents = 0

            for agent_id in list(self.registry.service_health.keys()):
                services_to_remove = []
                for service_name, last_heartbeat in self.registry.service_health[agent_id].items():
                    if last_heartbeat < cutoff_time:
                        services_to_remove.append(service_name)

                # 移除过期的服务记录
                for service_name in services_to_remove:
                    del self.registry.service_health[agent_id][service_name]
                    cleaned_services += 1

                # 如果agent下没有服务了，移除agent记录
                if not self.registry.service_health[agent_id]:
                    del self.registry.service_health[agent_id]
                    cleaned_agents += 1

            # 清理智能重连管理器中的过期和无效条目
            valid_client_ids = set(self.client_manager.get_all_clients().keys())
            cleaned_invalid_clients = self.smart_reconnection.cleanup_invalid_clients(valid_client_ids)
            cleaned_expired_entries = self.smart_reconnection.cleanup_expired_entries()

            if cleaned_services > 0 or cleaned_agents > 0 or cleaned_invalid_clients > 0 or cleaned_expired_entries > 0:
                logger.info(f"Cleanup completed: removed {cleaned_services} expired heartbeat records, "
                          f"{cleaned_agents} empty agent records, {cleaned_invalid_clients} invalid client entries, "
                          f"{cleaned_expired_entries} expired reconnection entries")
            else:
                logger.debug("Cleanup completed: no expired records found")

        except Exception as e:
            logger.error(f"Error during resource cleanup: {e}")

    async def connect_service(self, name: str, url: str = None) -> Tuple[bool, str]:
        """
        连接到指定的服务（支持本地和远程服务）

        Args:
            name: 服务名称
            url: 服务URL（可选，如果不提供则从配置中获取）

        Returns:
            Tuple[bool, str]: (是否成功, 消息)
        """
        try:
            # 获取服务配置
            service_config = self.mcp_config.get_service_config(name)
            if not service_config:
                return False, f"Service configuration not found for {name}"

            # 如果提供了URL，更新配置
            if url:
                service_config["url"] = url

            # 判断是本地服务还是远程服务
            if "command" in service_config:
                # 本地服务：先启动进程，再连接
                return await self._connect_local_service(name, service_config)
            else:
                # 远程服务：直接连接
                return await self._connect_remote_service(name, service_config)

        except Exception as e:
            logger.error(f"Failed to connect service {name}: {e}")
            return False, str(e)

    async def _connect_local_service(self, name: str, service_config: Dict[str, Any]) -> Tuple[bool, str]:
        """连接本地服务"""
        try:
            # 1. 启动本地服务进程
            success, message = await self.local_service_manager.start_local_service(name, service_config)
            if not success:
                return False, f"Failed to start local service: {message}"

            # 2. 等待服务启动
            await asyncio.sleep(2)

            # 3. 创建客户端连接
            # 本地服务通常使用 stdio 传输
            local_config = service_config.copy()

            # 使用 ConfigProcessor 处理配置
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
                    logger.info(f"Local service {name} connected successfully with {len(tools)} tools")
                    self.clients[name] = client
                    return True, f"Local service connected successfully with {len(tools)} tools"
            except Exception as e:
                logger.error(f"Failed to connect to local service {name}: {e}")
                # 如果连接失败，停止本地服务
                await self.local_service_manager.stop_local_service(name)
                return False, f"Failed to connect to local service: {str(e)}"

        except Exception as e:
            logger.error(f"Error connecting local service {name}: {e}")
            return False, str(e)

    async def _connect_remote_service(self, name: str, service_config: Dict[str, Any]) -> Tuple[bool, str]:
        """连接远程服务"""
        try:
            # 创建新的客户端
            client = Client({"mcpServers": {name: service_config}})

            # 尝试连接
            try:
                await client.list_tools()
                self.clients[name] = client
                logger.info(f"Remote service {name} connected successfully")
                return True, "Remote service connected successfully"
            except Exception as e:
                logger.error(f"Failed to connect to remote service {name}: {e}")
                return False, str(e)

        except Exception as e:
            logger.error(f"Error connecting remote service {name}: {e}")
            return False, str(e)

    async def disconnect_service(self, url_or_name: str) -> bool:
        """从配置中移除服务并更新main_client"""
        logger.info(f"Removing service: {url_or_name}")

        # 查找要移除的服务名
        name_to_remove = None
        for name, server in self.main_config.get("mcpServers", {}).items():
            if name == url_or_name or server.get("url") == url_or_name:
                name_to_remove = name
                break

        if name_to_remove:
            # 从main_config中移除
            if name_to_remove in self.main_config["mcpServers"]:
                del self.main_config["mcpServers"][name_to_remove]

            # 从配置文件中移除
            ok = self.mcp_config.remove_service(name_to_remove)
            if not ok:
                logger.warning(f"Failed to remove service {name_to_remove} from configuration file")

            # 从registry中移除
            self.registry.remove_service(name_to_remove)

            # 重新创建main_client
            if self.main_config.get("mcpServers"):
                self.main_client = Client(self.main_config)

                # 更新所有agent_clients
                for agent_id in list(self.agent_clients.keys()):
                    self.agent_clients[agent_id] = Client(self.main_config)
                    logger.info(f"Updated client for agent {agent_id} after removing service")

            else:
                # 如果没有服务了，清除main_client
                self.main_client = None
                # 清除所有agent_clients
                self.agent_clients.clear()

            return True
        else:
            logger.warning(f"Service {url_or_name} not found in configuration.")
            return False

    async def refresh_services(self):
        """手动刷新所有服务连接（重新加载mcp.json）"""
        await self.load_from_config()

    async def is_service_healthy(self, name: str, client_id: Optional[str] = None) -> bool:
        """
        检查服务是否健康（优化版本，快速失败，带网络检测）

        Args:
            name: 服务名
            client_id: 可选的客户端ID，用于多客户端环境

        Returns:
            bool: 服务是否健康
        """
        try:
            # 优先使用已处理的client配置，如果没有则使用原始配置
            if client_id:
                client_config = self.client_manager.get_client_config(client_id)
                if client_config and name in client_config.get("mcpServers", {}):
                    # 使用已处理的client配置
                    service_config = client_config["mcpServers"][name]
                    fastmcp_config = client_config
                    logger.debug(f"Using processed client config for health check: {name}")
                else:
                    # 回退到原始配置
                    service_config = self.mcp_config.get_service_config(name)
                    if not service_config:
                        logger.debug(f"Service configuration not found for {name}")
                        return False

                    # 使用ConfigProcessor处理配置
                    user_config = {"mcpServers": {name: service_config}}
                    fastmcp_config = ConfigProcessor.process_user_config_for_fastmcp(user_config)
                    logger.debug(f"Health check config processed for {name}: {fastmcp_config}")

                    # 检查ConfigProcessor是否移除了服务（配置错误）
                    if name not in fastmcp_config.get("mcpServers", {}):
                        logger.warning(f"Service {name} removed by ConfigProcessor due to configuration errors")
                        return False
            else:
                # 没有client_id，使用原始配置
                service_config = self.mcp_config.get_service_config(name)
                if not service_config:
                    logger.debug(f"Service configuration not found for {name}")
                    return False

                # 使用ConfigProcessor处理配置
                user_config = {"mcpServers": {name: service_config}}
                fastmcp_config = ConfigProcessor.process_user_config_for_fastmcp(user_config)
                logger.debug(f"Health check config processed for {name}: {fastmcp_config}")

                # 检查ConfigProcessor是否移除了服务（配置错误）
                if name not in fastmcp_config.get("mcpServers", {}):
                    logger.warning(f"Service {name} removed by ConfigProcessor due to configuration errors")
                    return False

            # 快速网络连通性检查（仅对HTTP服务）
            if service_config.get("url"):
                if not await self._quick_network_check(service_config["url"]):
                    logger.debug(f"Quick network check failed for {name}")
                    return False

            # 创建新的客户端实例
            client = Client(fastmcp_config)

            try:
                # 使用更短的超时时间，快速失败
                timeout_seconds = min(self.http_timeout, 3)  # 最大3秒，更快失败
                async with asyncio.timeout(timeout_seconds):
                    async with client:
                        await client.ping()
                        return True
            except asyncio.TimeoutError:
                logger.debug(f"Health check timeout for {name} (client_id={client_id}) after {timeout_seconds}s")
                return False
            except ConnectionError as e:
                logger.debug(f"Connection error for {name} (client_id={client_id}): {e}")
                return False
            except FileNotFoundError as e:
                # 命令服务的文件不存在
                logger.debug(f"Command service file not found for {name} (client_id={client_id}): {e}")
                return False
            except PermissionError as e:
                # 权限错误
                logger.debug(f"Permission error for {name} (client_id={client_id}): {e}")
                return False
            except Exception as e:
                # 使用ConfigProcessor提供更友好的错误信息
                friendly_error = ConfigProcessor.get_user_friendly_error(str(e))

                # 检查是否是文件系统相关错误
                if self._is_filesystem_error(e):
                    logger.debug(f"Filesystem error for {name} (client_id={client_id}): {friendly_error}")
                # 检查是否是网络相关错误
                elif self._is_network_error(e):
                    logger.debug(f"Network error for {name} (client_id={client_id}): {friendly_error}")
                elif "validation errors" in str(e).lower():
                    # 配置验证错误通常是由于用户自定义字段，这是正常的
                    logger.debug(f"Configuration has user-defined fields for {name} (client_id={client_id}): {friendly_error}")
                    # 对于配置验证错误，我们认为服务是"可用但需要配置清理"的状态
                    # 不应该完全标记为失败，而是标记为需要注意
                    logger.info(f"Service {name} has configuration validation issues but may still be functional")
                else:
                    logger.debug(f"Health check failed for {name} (client_id={client_id}): {friendly_error}")
                return False
            finally:
                # 确保客户端被正确关闭
                try:
                    await client.close()
                except Exception:
                    pass  # 忽略关闭时的错误

        except Exception as e:
            logger.debug(f"Health check failed for {name} (client_id={client_id}): {e}")
            return False

    async def _quick_network_check(self, url: str) -> bool:
        """快速网络连通性检查"""
        try:
            import aiohttp
            from urllib.parse import urlparse

            parsed = urlparse(url)
            if not parsed.hostname:
                return True  # 无法解析主机名，跳过检查

            # 简单的TCP连接检查
            try:
                reader, writer = await asyncio.wait_for(
                    asyncio.open_connection(parsed.hostname, parsed.port or 80),
                    timeout=1.0  # 1秒超时
                )
                writer.close()
                await writer.wait_closed()
                return True
            except Exception:
                return False

        except ImportError:
            # 如果没有aiohttp，跳过网络检查
            return True
        except Exception:
            return False

    def _is_network_error(self, error: Exception) -> bool:
        """判断是否是网络相关错误"""
        error_str = str(error).lower()
        network_error_keywords = [
            'connection', 'network', 'timeout', 'unreachable',
            'refused', 'reset', 'dns', 'resolve', 'socket'
        ]
        return any(keyword in error_str for keyword in network_error_keywords)

    def _is_filesystem_error(self, error: Exception) -> bool:
        """判断是否是文件系统相关错误"""
        if isinstance(error, (FileNotFoundError, PermissionError, OSError, IOError)):
            return True

        error_str = str(error).lower()
        filesystem_error_keywords = [
            'no such file', 'file not found', 'permission denied',
            'access denied', 'directory not found', 'path not found'
        ]
        return any(keyword in error_str for keyword in filesystem_error_keywords)

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

    # async def process_unified_query(
    #     self,
    #     query: str,
    #     agent_id: Optional[str] = None,
    #     mode: str = "react",
    #     include_trace: bool = False
    # ) -> Union[str, Dict[str, Any]]:
    #     """处理统一查询"""
    #     # 获取或创建会话
    #     session = self.session_manager.get_or_create_session(agent_id)
    #
    #     if not session.tools:
    #         # 如果会话没有工具，加载所有可用工具
    #         for service_name, client in self.clients.items():
    #             try:
    #                 tools = await client.list_tools()
    #                 for tool in tools:
    #                     session.add_tool(tool.name, {
    #                         "name": tool.name,
    #                         "description": tool.description,
    #                         "inputSchema": tool.inputSchema if hasattr(tool, "inputSchema") else None
    #                     }, service_name)
    #                     session.add_service(service_name, client)
    #             except Exception as e:
    #                 logger.error(f"Failed to load tools from service {service_name}: {e}")
    #
    #     # 处理查询...
    #     return {"result": "query processed", "session_id": session.agent_id}

    async def execute_tool_fastmcp(
        self,
        service_name: str,
        tool_name: str,
        arguments: Dict[str, Any] = None,
        agent_id: Optional[str] = None,
        timeout: Optional[float] = None,
        progress_handler = None,
        raise_on_error: bool = True
    ) -> Any:
        """
        执行工具（FastMCP 标准）
        严格按照 FastMCP 官网标准执行工具调用

        Args:
            service_name: 服务名称
            tool_name: 工具名称（FastMCP 原始名称）
            arguments: 工具参数
            agent_id: Agent ID（可选）
            timeout: 超时时间（秒）
            progress_handler: 进度处理器
            raise_on_error: 是否在错误时抛出异常

        Returns:
            FastMCP CallToolResult 或提取的数据
        """
        from mcpstore.core.tool_resolver import FastMCPToolExecutor

        arguments = arguments or {}
        executor = FastMCPToolExecutor(default_timeout=timeout or 30.0)

        try:
            if agent_id:
                # Agent 模式：在指定 Agent 的客户端中查找服务
                client_ids = self.client_manager.get_agent_clients(agent_id)
                if not client_ids:
                    raise Exception(f"No clients found for agent {agent_id}")
            else:
                # Store 模式：在 main_client 的客户端中查找服务
                client_ids = self.client_manager.get_agent_clients(self.client_manager.main_client_id)
                if not client_ids:
                    raise Exception("No clients found in main_client")

            # 遍历客户端查找服务
            for client_id in client_ids:
                if self.registry.has_service(client_id, service_name):
                    try:
                        # 获取服务配置并创建客户端
                        service_config = self.mcp_config.get_service_config(service_name)
                        if not service_config:
                            logger.warning(f"Service configuration not found for {service_name}")
                            continue

                        # 标准化配置并创建 FastMCP 客户端
                        normalized_config = self._normalize_service_config(service_config)
                        client = Client({"mcpServers": {service_name: normalized_config}})

                        async with client:
                            # 验证工具存在
                            tools = await client.list_tools()
                            if not any(t.name == tool_name for t in tools):
                                logger.warning(f"Tool {tool_name} not found in service {service_name}")
                                continue

                            # 使用 FastMCP 标准执行器执行工具
                            result = await executor.execute_tool(
                                client=client,
                                tool_name=tool_name,
                                arguments=arguments,
                                timeout=timeout,
                                progress_handler=progress_handler,
                                raise_on_error=raise_on_error
                            )

                            # 提取结果数据（按照 FastMCP 标准）
                            extracted_data = executor.extract_result_data(result)

                            logger.info(f"Tool {tool_name} executed successfully in service {service_name}")
                            return extracted_data

                    except Exception as e:
                        logger.error(f"Failed to execute tool in client {client_id}: {e}")
                        if raise_on_error:
                            raise
                        continue

            raise Exception(f"Tool {tool_name} not found in service {service_name}")

        except Exception as e:
            logger.error(f"FastMCP tool execution failed: {e}")
            raise Exception(f"Tool execution failed: {str(e)}")

    async def execute_tool(
        self,
        service_name: str,
        tool_name: str,
        parameters: Dict[str, Any],
        agent_id: Optional[str] = None
    ) -> Any:
        """
        执行工具（旧版本，已废弃）

        ⚠️ 此方法已废弃，请使用 execute_tool_fastmcp() 方法
        该方法保留仅为向后兼容，将在未来版本中移除
        """
        logger.warning("execute_tool() is deprecated, use execute_tool_fastmcp() instead")
        try:
            if agent_id:
                # agent模式：在agent的所有client中查找服务
                client_ids = self.client_manager.get_agent_clients(agent_id)
                if not client_ids:
                    raise Exception(f"No clients found for agent {agent_id}")
                    
                # 在所有client中查找服务
                for client_id in client_ids:
                    if self.registry.has_service(client_id, service_name):
                        # 获取服务配置
                        service_config = self.mcp_config.get_service_config(service_name)
                        if not service_config:
                            logger.warning(f"Service configuration not found for {service_name}")
                            continue
                            
                        logger.debug(f"Creating new client for service {service_name} with config: {service_config}")
                        # 确保配置包含transport字段（自动推断）
                        normalized_config = self._normalize_service_config(service_config)
                        # 创建新的客户端实例
                        client = Client({"mcpServers": {service_name: normalized_config}})
                        try:
                            async with client:
                                logger.debug(f"Client connected: {client.is_connected()}")
                                
                                # 获取工具列表并打印
                                tools = await client.list_tools()
                                logger.debug(f"Available tools for service {service_name}: {[t.name for t in tools]}")
                                
                                # 检查工具名称格式
                                base_tool_name = tool_name
                                if tool_name.startswith(f"{service_name}_"):
                                    base_tool_name = tool_name[len(service_name)+1:]
                                logger.debug(f"Using base tool name: {base_tool_name}")
                                
                                # 检查工具是否存在
                                if not any(t.name == base_tool_name for t in tools):
                                    logger.warning(f"Tool {base_tool_name} not found in available tools")
                                    continue
                                
                                # 执行工具
                                logger.debug(f"Calling tool {base_tool_name} with parameters: {parameters}")
                                result = await client.call_tool(base_tool_name, parameters)
                                logger.info(f"Tool {base_tool_name} executed successfully with client {client_id}")
                                return result
                        except Exception as e:
                            logger.error(f"Failed to execute tool with client {client_id}: {e}")
                            continue
                                
                raise Exception(f"Service {service_name} not found in any client for agent {agent_id}")
            else:
                # store模式：在main_client的所有client中查找服务
                client_ids = self.client_manager.get_agent_clients(self.client_manager.main_client_id)
                if not client_ids:
                    raise Exception("No clients found in main_client")
                    
                # 在所有client中查找服务
                for client_id in client_ids:
                    if self.registry.has_service(client_id, service_name):
                        # 获取服务配置
                        service_config = self.mcp_config.get_service_config(service_name)
                        if not service_config:
                            logger.warning(f"Service configuration not found for {service_name}")
                            continue
                            
                        logger.debug(f"Creating new client for service {service_name} with config: {service_config}")
                        # 确保配置包含transport字段（自动推断）
                        normalized_config = self._normalize_service_config(service_config)
                        # 创建新的客户端实例
                        client = Client({"mcpServers": {service_name: normalized_config}})
                        try:
                            async with client:
                                logger.debug(f"Client connected: {client.is_connected()}")
                                
                                # 获取工具列表并打印
                                tools = await client.list_tools()
                                logger.debug(f"Available tools for service {service_name}: {[t.name for t in tools]}")
                                
                                # 检查工具名称格式
                                base_tool_name = tool_name
                                if tool_name.startswith(f"{service_name}_"):
                                    base_tool_name = tool_name[len(service_name)+1:]
                                logger.debug(f"Using base tool name: {base_tool_name}")
                                
                                # 检查工具是否存在
                                if not any(t.name == base_tool_name for t in tools):
                                    logger.warning(f"Tool {base_tool_name} not found in available tools")
                                    continue
                                
                                # 执行工具
                                logger.debug(f"Calling tool {base_tool_name} with parameters: {parameters}")
                                result = await client.call_tool(base_tool_name, parameters)
                                logger.info(f"Tool {base_tool_name} executed successfully with client {client_id}")
                                return result
                        except Exception as e:
                            logger.error(f"Failed to execute tool with client {client_id}: {e}")
                            continue
                                
                raise Exception(f"Tool not found: {tool_name}")
        except Exception as e:
            logger.error(f"Tool execution failed: {e}")
            raise Exception(f"Tool execution failed: {str(e)}")

    async def cleanup(self):
        """清理资源"""
        logger.info("Cleaning up MCP Orchestrator resources...")

        # 清理会话
        self.session_manager.cleanup_expired_sessions()

        # 停止所有监控任务
        tasks_to_cancel = [
            ("heartbeat", self.heartbeat_task),
            ("reconnection", self.reconnection_task),
            ("cleanup", self.cleanup_task)
        ]

        for task_name, task in tasks_to_cancel:
            if task and not task.done():
                logger.debug(f"Cancelling {task_name} task...")
                task.cancel()
                try:
                    await task
                except asyncio.CancelledError:
                    logger.debug(f"{task_name} task cancelled successfully")
                except Exception as e:
                    logger.warning(f"Error cancelling {task_name} task: {e}")

        # 关闭所有客户端连接
        for name, client in self.clients.items():
            try:
                await client.close()
            except Exception as e:
                logger.error(f"Error closing client {name}: {e}")

        # 清理所有状态
        self.clients.clear()
        # 清理智能重连管理器
        self.smart_reconnection.entries.clear()

        logger.info("MCP Orchestrator cleanup completed")

    async def _restart_monitoring_tasks(self):
        """重启监控任务以应用新配置"""
        logger.info("Restarting monitoring tasks with new configuration...")

        # 停止现有任务
        tasks_to_stop = [
            ("heartbeat", self.heartbeat_task),
            ("reconnection", self.reconnection_task),
            ("cleanup", self.cleanup_task)
        ]

        for task_name, task in tasks_to_stop:
            if task and not task.done():
                logger.debug(f"Stopping {task_name} task...")
                task.cancel()
                try:
                    await task
                except asyncio.CancelledError:
                    logger.debug(f"{task_name} task stopped successfully")
                except Exception as e:
                    logger.warning(f"Error stopping {task_name} task: {e}")

        # 重新启动监控
        await self.start_monitoring()
        logger.info("Monitoring tasks restarted successfully")

    def _validate_configuration(self) -> bool:
        """验证配置完整性"""
        try:
            # 检查基本配置
            if not hasattr(self, 'mcp_config') or self.mcp_config is None:
                logger.error("MCP configuration is missing")
                return False

            # 检查时间间隔配置
            if self.heartbeat_interval.total_seconds() <= 0:
                logger.error("Invalid heartbeat interval")
                return False

            if self.reconnection_interval.total_seconds() <= 0:
                logger.error("Invalid reconnection interval")
                return False

            if self.cleanup_interval.total_seconds() <= 0:
                logger.error("Invalid cleanup interval")
                return False

            # 检查客户端管理器
            if not hasattr(self, 'client_manager') or self.client_manager is None:
                logger.error("Client manager is missing")
                return False

            # 检查注册表
            if not hasattr(self, 'registry') or self.registry is None:
                logger.error("Service registry is missing")
                return False

            # 检查智能重连管理器
            if not hasattr(self, 'smart_reconnection') or self.smart_reconnection is None:
                logger.error("Smart reconnection manager is missing")
                return False

            logger.debug("Configuration validation passed")
            return True

        except Exception as e:
            logger.error(f"Configuration validation failed: {e}")
            return False

    async def _heartbeat_loop_with_error_handling(self):
        """带错误处理的心跳循环"""
        consecutive_failures = 0
        max_consecutive_failures = 5

        while True:
            try:
                await asyncio.sleep(self.heartbeat_interval.total_seconds())
                await self._check_services_health()
                consecutive_failures = 0  # 重置失败计数

            except asyncio.CancelledError:
                logger.info("Heartbeat loop cancelled")
                break
            except Exception as e:
                consecutive_failures += 1
                logger.error(f"Heartbeat loop error (failure {consecutive_failures}/{max_consecutive_failures}): {e}")

                if consecutive_failures >= max_consecutive_failures:
                    logger.critical("Too many consecutive heartbeat failures, stopping heartbeat loop")
                    break

                # 指数退避延迟
                backoff_delay = min(60 * (2 ** consecutive_failures), 300)  # 最大5分钟
                await asyncio.sleep(backoff_delay)

    async def _reconnection_loop_with_error_handling(self):
        """带错误处理的重连循环"""
        consecutive_failures = 0
        max_consecutive_failures = 5

        while True:
            try:
                await asyncio.sleep(self.reconnection_interval.total_seconds())
                await self._attempt_reconnections()
                consecutive_failures = 0  # 重置失败计数

            except asyncio.CancelledError:
                logger.info("Reconnection loop cancelled")
                break
            except Exception as e:
                consecutive_failures += 1
                logger.error(f"Reconnection loop error (failure {consecutive_failures}/{max_consecutive_failures}): {e}")

                if consecutive_failures >= max_consecutive_failures:
                    logger.critical("Too many consecutive reconnection failures, stopping reconnection loop")
                    break

                # 指数退避延迟
                backoff_delay = min(60 * (2 ** consecutive_failures), 300)  # 最大5分钟
                await asyncio.sleep(backoff_delay)

    async def _cleanup_loop_with_error_handling(self):
        """带错误处理的清理循环"""
        consecutive_failures = 0
        max_consecutive_failures = 3

        while True:
            try:
                await asyncio.sleep(self.cleanup_interval.total_seconds())
                await self._perform_cleanup()
                consecutive_failures = 0  # 重置失败计数

            except asyncio.CancelledError:
                logger.info("Cleanup loop cancelled")
                break
            except Exception as e:
                consecutive_failures += 1
                logger.error(f"Cleanup loop error (failure {consecutive_failures}/{max_consecutive_failures}): {e}")

                if consecutive_failures >= max_consecutive_failures:
                    logger.critical("Too many consecutive cleanup failures, stopping cleanup loop")
                    break

                # 较长的退避延迟（清理不那么关键）
                backoff_delay = min(300 * (2 ** consecutive_failures), 1800)  # 最大30分钟
                await asyncio.sleep(backoff_delay)

    async def register_agent_client(self, agent_id: str, config: Optional[Dict[str, Any]] = None) -> Client:
        """
        为agent注册一个新的client实例

        Args:
            agent_id: 代理ID
            config: 可选的配置，如果为None则使用main_config

        Returns:
            新创建的Client实例
        """
        # 使用main_config或提供的config创建新的client
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
        过滤出健康的服务列表

        Args:
            services: 服务名列表
            client_id: 可选的客户端ID，用于多客户端环境

        Returns:
            List[str]: 健康的服务名列表
        """
        healthy_services = []
        for name in services:
            try:
                service_config = self.mcp_config.get_service_config(name)
                if not service_config:
                    logger.warning(f"Service configuration not found for {name}")
                    continue

                # 确保配置包含transport字段（自动推断）
                normalized_config = self._normalize_service_config(service_config)
                # 创建新的客户端实例
                client = Client({"mcpServers": {name: normalized_config}})
                
                try:
                    # 使用超时控制的异步上下文管理器
                    async with asyncio.timeout(self.http_timeout):
                        async with client:
                            await client.ping()
                            healthy_services.append(name)
                except asyncio.TimeoutError:
                    logger.warning(f"Health check timeout for {name} (client_id={client_id})")
                    continue
                except Exception as e:
                    logger.warning(f"Health check failed for {name} (client_id={client_id}): {e}")
                    continue
                finally:
                    # 确保客户端被正确关闭
                    try:
                        await client.close()
                    except Exception:
                        pass  # 忽略关闭时的错误
                        
            except Exception as e:
                logger.warning(f"Health check failed for {name} (client_id={client_id}): {e}")
                continue

        return healthy_services

    async def start_main_client(self, config: Dict[str, Any]):
        """启动 main_client 的 async with 生命周期，注册服务和工具（仅健康服务）"""
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
        await self.register_json_services(healthy_config, client_id="main_client")
        # main_client专属管理逻辑可在这里补充（如缓存、生命周期等）

    async def register_json_services(self, config: Dict[str, Any], client_id: str = None, agent_id: str = None):
        """注册JSON配置中的服务（可用于main_client或普通client）"""
        # agent_id 兼容
        agent_key = agent_id or client_id or self.client_manager.main_client_id
        try:
            # 获取健康的服务列表
            healthy_services = await self.filter_healthy_services(list(config.get("mcpServers", {}).keys()), client_id)
            
            if not healthy_services:
                logger.warning("No healthy services found")
                return {
                    "client_id": client_id or "main_client",
                    "services": {},
                    "total_success": 0,
                    "total_failed": 0
                }

            # 使用healthy_services构建新的配置
            healthy_config = {
                "mcpServers": {
                    name: config["mcpServers"][name]
                    for name in healthy_services
                }
            }

            # 使用ConfigProcessor处理配置，确保FastMCP兼容性
            logger.debug(f"Processing config for FastMCP compatibility: {list(healthy_config['mcpServers'].keys())}")
            fastmcp_config = ConfigProcessor.process_user_config_for_fastmcp(healthy_config)
            logger.debug(f"Config processed for FastMCP: {fastmcp_config}")

            # 使用处理后的配置创建客户端
            client = Client(fastmcp_config)

            try:
                async with client:
                    # 获取工具列表
                    tool_list = await client.list_tools()
                    if not tool_list:
                        logger.warning("No tools found")
                        return {
                            "client_id": client_id or "main_client",
                            "services": {},
                            "total_success": 0,
                            "total_failed": 0
                        }

                    # 处理工具列表
                    all_tools = []
                    
                    # 判断是否是单服务情况
                    is_single_service = len(healthy_services) == 1
                    
                    for tool in tool_list:
                        original_tool_name = tool.name

                        # 🆕 使用统一的工具命名标准
                        from mcpstore.core.tool_resolver import ToolNameResolver

                        if is_single_service:
                            # 单服务情况：直接使用原始工具名，记录服务归属
                            service_name = healthy_services[0]
                            display_name = ToolNameResolver().create_user_friendly_name(service_name, original_tool_name)
                            logger.debug(f"Single service tool: {original_tool_name} -> display as {display_name}")
                        else:
                            # 多服务情况：为每个服务分别注册工具
                            service_name = healthy_services[0]  # 默认分配给第一个服务
                            display_name = ToolNameResolver().create_user_friendly_name(service_name, original_tool_name)
                            logger.debug(f"Multi-service tool: {original_tool_name} -> assigned to {service_name} -> display as {display_name}")

                        # 处理参数信息
                        parameters = {}
                        if hasattr(tool, 'inputSchema') and tool.inputSchema:
                            parameters = tool.inputSchema
                        elif hasattr(tool, 'parameters') and tool.parameters:
                            parameters = tool.parameters

                        # 构造工具定义（存储显示名称和原始名称）
                        tool_def = {
                            "type": "function",
                            "function": {
                                "name": original_tool_name,  # FastMCP 原始名称
                                "display_name": display_name,  # 用户友好的显示名称
                                "description": tool.description,
                                "parameters": parameters,
                                "service_name": service_name  # 明确的服务归属
                            }
                        }
                        # 使用显示名称作为存储键，这样用户输入的显示名称可以直接匹配
                        all_tools.append((display_name, tool_def, service_name))

                    # 🆕 为每个服务注册其工具（使用统一的标准）
                    for service_name in healthy_services:
                        # 筛选属于该服务的工具
                        service_tools = []
                        for tool_name, tool_def, tool_service in all_tools:
                            if tool_service == service_name:
                                # 存储格式：(原始名称, 工具定义)
                                service_tools.append((tool_name, tool_def))

                        logger.info(f"Registering {len(service_tools)} tools for service {service_name}")
                        self.registry.add_service(agent_key, service_name, client, service_tools)
                        self.clients[service_name] = client

                    return {
                        "client_id": client_id or "main_client",
                        "services": {
                            name: {"status": "success", "message": "Service registered successfully"}
                            for name in healthy_services
                        },
                        "total_success": len(healthy_services),
                        "total_failed": 0
                    }
            except Exception as e:
                logger.error(f"Error retrieving tools: {e}", exc_info=True)
                return {
                    "client_id": client_id or "main_client",
                    "services": {},
                    "total_success": 0,
                    "total_failed": 1,
                    "error": str(e)
                }
        except Exception as e:
            logger.error(f"Error registering services: {e}", exc_info=True)
            return {
                "client_id": client_id or "main_client",
                "services": {},
                "total_success": 0,
                "total_failed": 1,
                "error": str(e)
            }

    def create_client_config_from_names(self, service_names: list) -> Dict[str, Any]:
        """
        根据服务名列表，从 mcp.json 生成新的 client config
        """
        all_services = self.mcp_config.load_config().get("mcpServers", {})
        selected = {name: all_services[name] for name in service_names if name in all_services}
        return {"mcpServers": selected}

    def remove_service(self, service_name: str, agent_id: str = None):
        agent_key = agent_id or self.client_manager.main_client_id
        self.registry.remove_service(agent_key, service_name)
        # ...其余逻辑...

    def get_session(self, service_name: str, agent_id: str = None):
        agent_key = agent_id or self.client_manager.main_client_id
        return self.registry.get_session(agent_key, service_name)

    def get_tools_for_service(self, service_name: str, agent_id: str = None):
        agent_key = agent_id or self.client_manager.main_client_id
        return self.registry.get_tools_for_service(agent_key, service_name)

    def get_all_service_names(self, agent_id: str = None):
        agent_key = agent_id or self.client_manager.main_client_id
        return self.registry.get_all_service_names(agent_key)

    def get_all_tool_info(self, agent_id: str = None):
        agent_key = agent_id or self.client_manager.main_client_id
        return self.registry.get_all_tool_info(agent_key)

    def get_service_details(self, service_name: str, agent_id: str = None):
        agent_key = agent_id or self.client_manager.main_client_id
        return self.registry.get_service_details(agent_key, service_name)

    def update_service_health(self, service_name: str, agent_id: str = None):
        agent_key = agent_id or self.client_manager.main_client_id
        self.registry.update_service_health(agent_key, service_name)

    def get_last_heartbeat(self, service_name: str, agent_id: str = None):
        agent_key = agent_id or self.client_manager.main_client_id
        return self.registry.get_last_heartbeat(agent_key, service_name)

    def has_service(self, service_name: str, agent_id: str = None):
        agent_key = agent_id or self.client_manager.main_client_id
        return self.registry.has_service(agent_key, service_name)
