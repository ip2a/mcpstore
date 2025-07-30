import os
import sys

sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

"""
MCP服务编排器

该模块提供了MCPOrchestrator类，用于管理MCP服务的连接、工具调用和查询处理。
它是FastAPI应用程序的核心组件，负责协调客户端和服务之间的交互。
"""

import asyncio
import logging
import time
from typing import Dict, List, Any, Optional, Tuple
from datetime import datetime, timedelta

from mcpstore.core.registry import ServiceRegistry
from mcpstore.core.client_manager import ClientManager
from mcpstore.core.config_processor import ConfigProcessor
from mcpstore.core.local_service_manager import get_local_service_manager
from fastmcp import Client
from mcpstore.config.json_config import MCPConfig
from mcpstore.core.session_manager import SessionManager
# from mcpstore.core.smart_reconnection import SmartReconnectionManager  # 已废弃
from mcpstore.core.health_manager import get_health_manager, HealthStatus, HealthCheckResult
from mcpstore.core.service_lifecycle_manager import ServiceLifecycleManager
from mcpstore.core.service_content_manager import ServiceContentManager
from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)

class MCPOrchestrator:
    """
    MCP服务编排器

    负责管理服务连接、工具调用和查询处理。
    """

    def __init__(self, config: Dict[str, Any], registry: ServiceRegistry, standalone_config_manager=None, client_services_path=None, mcp_config=None):
        """
        初始化MCP编排器

        Args:
            config: 配置字典
            registry: 服务注册表实例
            standalone_config_manager: 独立配置管理器（可选）
            client_services_path: 客户端服务配置文件路径（可选，用于数据空间）
            mcp_config: MCPConfig实例（可选，用于数据空间）
        """
        self.config = config
        self.registry = registry
        self.clients: Dict[str, Client] = {}  # key为mcpServers的服务名
        self.main_client: Optional[Client] = None
        self.main_client_ctx = None  # async context manager for main_client
        self.main_config = {"mcpServers": {}}  # 中央配置
        self.agent_clients: Dict[str, Client] = {}  # agent_id -> client映射
        # 旧的智能重连管理器已被ServiceLifecycleManager替代
        # self.smart_reconnection = SmartReconnectionManager()  # 已废弃
        self.react_agent = None

        # 🔧 新增：独立配置管理器
        self.standalone_config_manager = standalone_config_manager

        # 旧的心跳和重连配置已被ServiceLifecycleManager替代
        timing_config = config.get("timing", {})
        # 保留http_timeout，其他配置已废弃
        self.http_timeout = int(timing_config.get("http_timeout_seconds", 10))

        # 旧的监控任务已被ServiceLifecycleManager替代
        # self.heartbeat_task = None      # 已废弃
        # self.reconnection_task = None   # 已废弃
        # self.cleanup_task = None        # 已废弃

        # 🔧 修改：根据是否有独立配置管理器或传入的mcp_config决定如何初始化MCPConfig
        if standalone_config_manager:
            # 使用独立配置，不依赖文件系统
            self.mcp_config = self._create_standalone_mcp_config(standalone_config_manager)
        elif mcp_config:
            # 使用传入的MCPConfig实例（用于数据空间）
            self.mcp_config = mcp_config
        else:
            # 使用传统配置
            self.mcp_config = MCPConfig()

        # 旧的资源管理配置已被ServiceLifecycleManager替代
        # 保留一些配置以避免错误，但实际不再使用

        # 客户端管理器 - 支持数据空间
        self.client_manager = ClientManager(services_path=client_services_path)

        # 会话管理器
        self.session_manager = SessionManager()

        # 本地服务管理器
        self.local_service_manager = get_local_service_manager()

        # 健康管理器
        self.health_manager = get_health_manager()

        # 服务生命周期管理器
        self.lifecycle_manager = ServiceLifecycleManager(self)

        # 服务内容管理器（替代旧的工具更新监控器）
        self.content_manager = ServiceContentManager(self)

        # 旧的工具更新监控器（保留兼容性，但将被废弃）
        self.tools_update_monitor = None

    async def setup(self):
        """初始化编排器资源（不再做服务注册）"""
        logger.info("Setting up MCP Orchestrator...")

        # 初始化健康管理器配置
        self._update_health_manager_config()

        # 初始化工具更新监控器
        self._setup_tools_update_monitor()

        # 启动生命周期管理器
        await self.lifecycle_manager.start()

        # 启动内容管理器
        await self.content_manager.start()

        # 只做必要的资源初始化
        logger.info("MCP Orchestrator setup completed with lifecycle and content management")

    async def shutdown(self):
        """关闭编排器并清理资源"""
        logger.info("Shutting down MCP Orchestrator...")

        # 停止生命周期管理器
        await self.lifecycle_manager.stop()

        # 停止内容管理器
        await self.content_manager.stop()

        # 旧的后台任务已被废弃，无需停止
        logger.info("Legacy monitoring tasks were already disabled")

        logger.info("MCP Orchestrator shutdown completed")

    def _update_health_manager_config(self):
        """更新健康管理器配置"""
        try:
            # 从配置中提取健康相关设置
            timing_config = self.config.get("timing", {})

            # 构建健康管理器配置
            health_config = {
                "local_service_ping_timeout": timing_config.get("local_service_ping_timeout", 3),
                "remote_service_ping_timeout": timing_config.get("remote_service_ping_timeout", 5),
                "startup_wait_time": timing_config.get("startup_wait_time", 2),
                "healthy_response_threshold": timing_config.get("healthy_response_threshold", 1.0),
                "warning_response_threshold": timing_config.get("warning_response_threshold", 3.0),
                "slow_response_threshold": timing_config.get("slow_response_threshold", 10.0),
                "enable_adaptive_timeout": timing_config.get("enable_adaptive_timeout", False),
                "adaptive_timeout_multiplier": timing_config.get("adaptive_timeout_multiplier", 2.0),
                "response_time_history_size": timing_config.get("response_time_history_size", 10)
            }

            # 更新健康管理器配置
            self.health_manager.update_config(health_config)
            logger.info(f"Health manager configuration updated: {health_config}")

        except Exception as e:
            logger.warning(f"Failed to update health manager config: {e}")

    def update_monitoring_config(self, monitoring_config: Dict[str, Any]):
        """更新监控配置（包括健康检查配置）"""
        try:
            # 更新时间配置
            if "timing" not in self.config:
                self.config["timing"] = {}

            # 映射监控配置到时间配置
            timing_mapping = {
                "local_service_ping_timeout": "local_service_ping_timeout",
                "remote_service_ping_timeout": "remote_service_ping_timeout",
                "startup_wait_time": "startup_wait_time",
                "healthy_response_threshold": "healthy_response_threshold",
                "warning_response_threshold": "warning_response_threshold",
                "slow_response_threshold": "slow_response_threshold",
                "enable_adaptive_timeout": "enable_adaptive_timeout",
                "adaptive_timeout_multiplier": "adaptive_timeout_multiplier",
                "response_time_history_size": "response_time_history_size"
            }

            for monitor_key, timing_key in timing_mapping.items():
                if monitor_key in monitoring_config and monitoring_config[monitor_key] is not None:
                    self.config["timing"][timing_key] = monitoring_config[monitor_key]

            # 更新健康管理器配置
            self._update_health_manager_config()

            logger.info("Monitoring configuration updated successfully")

        except Exception as e:
            logger.error(f"Failed to update monitoring config: {e}")
            raise

    def _setup_tools_update_monitor(self):
        """设置工具更新监控器"""
        try:
            from mcpstore.core.tools_update_monitor import ToolsUpdateMonitor
            self.tools_update_monitor = ToolsUpdateMonitor(self)
            logger.info("Tools update monitor initialized")
        except Exception as e:
            logger.error(f"Failed to setup tools update monitor: {e}")

    async def cleanup(self):
        """清理编排器资源"""
        logger.info("Cleaning up MCP Orchestrator...")

        # 停止工具更新监控器
        if self.tools_update_monitor:
            await self.tools_update_monitor.stop()

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
        """
        启动监控任务 - 已重构为使用ServiceLifecycleManager
        旧的心跳、重连、清理任务已被生命周期管理器替代
        """
        logger.info("Monitoring is now handled by ServiceLifecycleManager")
        logger.info("Legacy heartbeat and reconnection tasks have been disabled")

        # 只启动工具更新监控器（这个还需要保留）
        if self.tools_update_monitor:
            await self.tools_update_monitor.start()
            logger.info("Tools update monitor started")

        return True

    async def _heartbeat_loop(self):
        """
        后台循环，用于定期健康检查
        ⚠️ 已废弃：此方法已被ServiceLifecycleManager替代
        """
        logger.warning("_heartbeat_loop is deprecated and replaced by ServiceLifecycleManager")
        return

    async def _check_services_health(self):
        """
        并发检查所有服务的健康状态
        ⚠️ 已废弃：此方法已被ServiceLifecycleManager替代
        """
        logger.warning("_check_services_health is deprecated and replaced by ServiceLifecycleManager")
        return

    async def _check_single_service_health(self, name: str, client_id: str) -> bool:
        """检查单个服务的健康状态并更新生命周期状态"""
        try:
            # 执行详细健康检查
            health_result = await self.check_service_health_detailed(name, client_id)
            is_healthy = health_result.status != HealthStatus.UNHEALTHY

            # 旧的健康状态更新已废弃，现在完全由生命周期管理器处理

            # 通知生命周期管理器处理健康检查结果
            await self.lifecycle_manager.handle_health_check_result(
                agent_id=client_id,
                service_name=name,
                success=is_healthy,
                response_time=health_result.response_time,
                error_message=health_result.error_message
            )

            if is_healthy:
                logger.debug(f"Health check SUCCESS for: {name} (client_id={client_id})")
                return True
            else:
                logger.debug(f"Health check FAILED for {name} (client_id={client_id}): {health_result.error_message}")
                return False

        except Exception as e:
            logger.warning(f"Health check error for {name} (client_id={client_id}): {e}")
            # 通知生命周期管理器处理错误
            await self.lifecycle_manager.handle_health_check_result(
                agent_id=client_id,
                service_name=name,
                success=False,
                response_time=0.0,
                error_message=str(e)
            )
            return False

    async def _reconnection_loop(self):
        """
        定期尝试重新连接服务的后台循环
        ⚠️ 已废弃：此方法已被ServiceLifecycleManager替代
        """
        logger.warning("_reconnection_loop is deprecated and replaced by ServiceLifecycleManager")
        return

    async def _attempt_reconnections(self):
        """
        尝试重新连接所有待重连的服务（智能重连策略）
        ⚠️ 已废弃：此方法已被ServiceLifecycleManager替代
        """
        logger.warning("_attempt_reconnections is deprecated and replaced by ServiceLifecycleManager")
        return

    async def _cleanup_loop(self):
        """
        定期资源清理循环
        ⚠️ 已废弃：此方法已被ServiceLifecycleManager替代
        """
        logger.warning("_cleanup_loop is deprecated and replaced by ServiceLifecycleManager")
        return

    async def _perform_cleanup(self):
        """
        执行资源清理
        ⚠️ 已废弃：此方法已被ServiceLifecycleManager替代
        """
        logger.warning("_perform_cleanup is deprecated and replaced by ServiceLifecycleManager")
        return

    async def connect_service(self, name: str, url: str = None, agent_id: str = None) -> Tuple[bool, str]:
        """
        连接到指定的服务（支持本地和远程服务）并更新缓存

        Args:
            name: 服务名称
            url: 服务URL（可选，如果不提供则从配置中获取）
            agent_id: Agent ID（可选，如果不提供则使用main_client_id）

        Returns:
            Tuple[bool, str]: (是否成功, 消息)
        """
        try:
            # 确定Agent ID
            agent_key = agent_id or self.client_manager.main_client_id

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

                    # 🔧 修复：更新Registry缓存
                    await self._update_service_cache(agent_id, name, client, tools, service_config)

                    # 更新客户端缓存（保持向后兼容）
                    self.clients[name] = client

                    logger.info(f"Local service {name} connected successfully with {len(tools)} tools for agent {agent_id}")
                    return True, f"Local service connected successfully with {len(tools)} tools"
            except Exception as e:
                logger.error(f"Failed to connect to local service {name}: {e}")
                # 如果连接失败，停止本地服务
                await self.local_service_manager.stop_local_service(name)
                return False, f"Failed to connect to local service: {str(e)}"

        except Exception as e:
            logger.error(f"Error connecting local service {name}: {e}")
            return False, str(e)

    async def _connect_remote_service(self, name: str, service_config: Dict[str, Any], agent_id: str) -> Tuple[bool, str]:
        """连接远程服务并更新缓存"""
        try:
            # 创建新的客户端
            client = Client({"mcpServers": {name: service_config}})

            # 尝试连接
            try:
                async with client:
                    tools = await client.list_tools()

                    # 🔧 修复：更新Registry缓存
                    await self._update_service_cache(agent_id, name, client, tools, service_config)

                    # 更新客户端缓存（保持向后兼容）
                    self.clients[name] = client

                    logger.info(f"Remote service {name} connected successfully with {len(tools)} tools for agent {agent_id}")
                    return True, f"Remote service connected successfully with {len(tools)} tools"
            except Exception as e:
                logger.error(f"Failed to connect to remote service {name}: {e}")
                return False, str(e)

        except Exception as e:
            logger.error(f"Error connecting remote service {name}: {e}")
            return False, str(e)

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
            # 清除旧缓存
            self.registry.remove_service(agent_id, service_name)

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

            # 添加到Registry缓存
            self.registry.add_service(agent_id, service_name, client, processed_tools)

            # 标记长连接服务
            if self._is_long_lived_service(service_config):
                self.registry.mark_as_long_lived(agent_id, service_name)

            # 通知生命周期管理器连接成功
            await self.lifecycle_manager.handle_health_check_result(
                agent_id=agent_id,
                service_name=service_name,
                success=True,
                response_time=0.0,  # 连接时间，可以后续优化
                error_message=None
            )

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
            from mcpstore.core.tool_resolver import ToolNameResolver
            resolver = ToolNameResolver()
            return resolver.create_user_friendly_name(service_name, original_tool_name)
        except Exception as e:
            logger.warning(f"Failed to generate display name for {original_tool_name}: {e}")
            # 回退到简单格式
            return f"{service_name}_{original_tool_name}"

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

    async def refresh_service_content(self, service_name: str, agent_id: str = None) -> bool:
        """手动刷新指定服务的内容（工具、资源、提示词）"""
        agent_key = agent_id or self.client_manager.main_client_id
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
        # 只有unhealthy才返回False，其他状态都认为是"可用的"
        return result.status != HealthStatus.UNHEALTHY

    async def check_service_health_detailed(self, name: str, client_id: Optional[str] = None) -> HealthCheckResult:
        """
        详细的服务健康检查，返回完整的健康状态信息

        Args:
            name: 服务名
            client_id: 可选的客户端ID，用于多客户端环境

        Returns:
            HealthCheckResult: 详细的健康检查结果
        """
        start_time = time.time()
        try:
            # 获取服务配置
            service_config, fastmcp_config = await self._get_service_config_for_health_check(name, client_id)
            if not service_config:
                error_msg = f"Service configuration not found for {name}"
                logger.debug(error_msg)
                return self.health_manager.record_health_check(
                    name, 0.0, False, error_msg, service_config
                )

            # 快速网络连通性检查（仅对HTTP服务）
            if service_config.get("url"):
                if not await self._quick_network_check(service_config["url"]):
                    error_msg = f"Quick network check failed for {name}"
                    logger.debug(error_msg)
                    response_time = time.time() - start_time
                    return self.health_manager.record_health_check(
                        name, response_time, False, error_msg, service_config
                    )

            # 获取智能调整的超时时间
            timeout_seconds = self.health_manager.get_service_timeout(name, service_config)
            logger.debug(f"Using timeout {timeout_seconds}s for service {name}")

            # 创建新的客户端实例
            client = Client(fastmcp_config)

            try:
                async with asyncio.timeout(timeout_seconds):
                    async with client:
                        await client.ping()
                        # 成功响应，记录响应时间
                        response_time = time.time() - start_time
                        return self.health_manager.record_health_check(
                            name, response_time, True, None, service_config
                        )
            except asyncio.TimeoutError:
                response_time = time.time() - start_time
                error_msg = f"Health check timeout after {timeout_seconds}s"
                logger.debug(f"{error_msg} for {name} (client_id={client_id})")
                return self.health_manager.record_health_check(
                    name, response_time, False, error_msg, service_config
                )
            except ConnectionError as e:
                response_time = time.time() - start_time
                error_msg = f"Connection error: {str(e)}"
                logger.debug(f"{error_msg} for {name} (client_id={client_id})")
                return self.health_manager.record_health_check(
                    name, response_time, False, error_msg, service_config
                )
            except FileNotFoundError as e:
                response_time = time.time() - start_time
                error_msg = f"Command service file not found: {str(e)}"
                logger.debug(f"{error_msg} for {name} (client_id={client_id})")
                return self.health_manager.record_health_check(
                    name, response_time, False, error_msg, service_config
                )
            except PermissionError as e:
                response_time = time.time() - start_time
                error_msg = f"Permission error: {str(e)}"
                logger.debug(f"{error_msg} for {name} (client_id={client_id})")
                return self.health_manager.record_health_check(
                    name, response_time, False, error_msg, service_config
                )
            except Exception as e:
                response_time = time.time() - start_time
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
                    logger.info(f"Service {name} has configuration validation issues but may still be functional")
                else:
                    logger.debug(f"Health check failed for {name} (client_id={client_id}): {friendly_error}")

                return self.health_manager.record_health_check(
                    name, response_time, False, friendly_error, service_config
                )
            finally:
                # 确保客户端被正确关闭
                try:
                    await client.close()
                except Exception:
                    pass  # 忽略关闭时的错误

        except Exception as e:
            response_time = time.time() - start_time
            error_msg = f"Health check failed: {str(e)}"
            logger.debug(f"{error_msg} for {name} (client_id={client_id})")
            return self.health_manager.record_health_check(
                name, response_time, False, error_msg, {}
            )

    def get_service_comprehensive_status(self, service_name: str, client_id: str = None) -> str:
        """获取服务的完整状态（包括重连状态）"""
        from mcpstore.core.monitoring_config import ServiceStatus

        if client_id is None:
            client_id = self.client_manager.main_client_id

        service_key = f"{client_id}:{service_name}"

        # 1. 检查是否在重连队列中
        if service_key in self.smart_reconnection.entries:
            entry = self.smart_reconnection.entries[service_key]

            # 检查是否正在重连
            from datetime import datetime
            now = datetime.now()
            if entry.next_attempt and entry.next_attempt <= now:
                return ServiceStatus.RECONNECTING.value
            else:
                return ServiceStatus.DISCONNECTED.value

        # 2. 检查健康状态
        if service_name in self.health_manager.service_trackers:
            tracker = self.health_manager.service_trackers[service_name]
            return tracker.current_status.value

        return ServiceStatus.UNKNOWN.value

    async def _get_service_config_for_health_check(self, name: str, client_id: Optional[str] = None) -> Tuple[Optional[Dict[str, Any]], Optional[Dict[str, Any]]]:
        """获取用于健康检查的服务配置"""
        try:
            # 优先使用已处理的client配置，如果没有则使用原始配置
            if client_id:
                client_config = self.client_manager.get_client_config(client_id)
                if client_config and name in client_config.get("mcpServers", {}):
                    # 使用已处理的client配置
                    service_config = client_config["mcpServers"][name]
                    fastmcp_config = client_config
                    logger.debug(f"Using processed client config for health check: {name}")
                    return service_config, fastmcp_config
                else:
                    # 回退到原始配置
                    service_config = self.mcp_config.get_service_config(name)
                    if not service_config:
                        return None, None

                    # 使用ConfigProcessor处理配置
                    user_config = {"mcpServers": {name: service_config}}
                    fastmcp_config = ConfigProcessor.process_user_config_for_fastmcp(user_config)
                    logger.debug(f"Health check config processed for {name}: {fastmcp_config}")

                    # 检查ConfigProcessor是否移除了服务（配置错误）
                    if name not in fastmcp_config.get("mcpServers", {}):
                        logger.warning(f"Service {name} removed by ConfigProcessor due to configuration errors")
                        return None, None

                    return service_config, fastmcp_config
            else:
                # 没有client_id，使用原始配置
                service_config = self.mcp_config.get_service_config(name)
                if not service_config:
                    return None, None

                # 使用ConfigProcessor处理配置
                user_config = {"mcpServers": {name: service_config}}
                fastmcp_config = ConfigProcessor.process_user_config_for_fastmcp(user_config)
                logger.debug(f"Health check config processed for {name}: {fastmcp_config}")

                # 检查ConfigProcessor是否移除了服务（配置错误）
                if name not in fastmcp_config.get("mcpServers", {}):
                    logger.warning(f"Service {name} removed by ConfigProcessor due to configuration errors")
                    return None, None

                return service_config, fastmcp_config
        except Exception as e:
            logger.error(f"Error getting service config for health check {name}: {e}")
            return None, None

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

        # 旧的监控任务已被废弃，无需停止
        logger.info("Legacy monitoring tasks were already disabled")

        # 关闭所有客户端连接
        for name, client in self.clients.items():
            try:
                await client.close()
            except Exception as e:
                logger.error(f"Error closing client {name}: {e}")

        # 清理所有状态
        self.clients.clear()
        # 智能重连管理器已被废弃，无需清理

        logger.info("MCP Orchestrator cleanup completed")

    async def _restart_monitoring_tasks(self):
        """重启监控任务以应用新配置"""
        logger.info("Restarting monitoring tasks with new configuration...")

        # 旧的监控任务已被废弃，无需停止
        logger.info("Legacy monitoring tasks were already disabled")

        # 重新启动监控（现在由ServiceLifecycleManager处理）
        await self.start_monitoring()
        logger.info("Monitoring tasks restarted successfully")

    def _validate_configuration(self) -> bool:
        """验证配置完整性"""
        try:
            # 检查基本配置
            if not hasattr(self, 'mcp_config') or self.mcp_config is None:
                logger.error("MCP configuration is missing")
                return False

            # 旧的时间间隔配置检查已废弃（现在由ServiceLifecycleManager管理）
            # 保留配置读取以避免错误，但不再验证
            logger.debug("Legacy heartbeat configuration validation skipped")

            # 清理间隔配置检查已废弃（现在由ServiceLifecycleManager管理）
            logger.debug("Legacy cleanup configuration validation skipped")

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
        """
        带错误处理的心跳循环
        ⚠️ 已废弃：此方法已被ServiceLifecycleManager替代
        """
        logger.warning("_heartbeat_loop_with_error_handling is deprecated and replaced by ServiceLifecycleManager")
        return

    async def _reconnection_loop_with_error_handling(self):
        """
        带错误处理的重连循环
        ⚠️ 已废弃：此方法已被ServiceLifecycleManager替代
        """
        logger.warning("_reconnection_loop_with_error_handling is deprecated and replaced by ServiceLifecycleManager")
        return

    async def _cleanup_loop_with_error_handling(self):
        """
        带错误处理的清理循环
        ⚠️ 已废弃：此方法已被ServiceLifecycleManager替代
        """
        logger.warning("_cleanup_loop_with_error_handling is deprecated and replaced by ServiceLifecycleManager")
        return

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
        过滤出健康的服务列表 - 使用生命周期管理器

        Args:
            services: 服务名列表
            client_id: 可选的客户端ID，用于多客户端环境

        Returns:
            List[str]: 健康的服务名列表
        """
        healthy_services = []
        agent_id = client_id or self.client_manager.main_client_id

        for name in services:
            try:
                # 使用生命周期管理器获取服务状态
                service_state = self.lifecycle_manager.get_service_state(agent_id, name)

                # 健康状态和初始化状态的服务都被认为是可处理的
                from mcpstore.core.models.service import ServiceConnectionState
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

                        # 初始化服务生命周期状态
                        service_config = config["mcpServers"].get(service_name, {})
                        self.lifecycle_manager.initialize_service(agent_key, service_name, service_config)

                        # 添加到内容监控
                        self.content_manager.add_service_for_monitoring(agent_key, service_name)

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

    async def remove_service(self, service_name: str, agent_id: str = None):
        """移除服务并处理生命周期状态"""
        agent_key = agent_id or self.client_manager.main_client_id

        # 通知生命周期管理器开始优雅断连
        await self.lifecycle_manager.graceful_disconnect(agent_key, service_name, "user_requested")

        # 从内容监控中移除
        self.content_manager.remove_service_from_monitoring(agent_key, service_name)

        # 从注册表中移除服务
        self.registry.remove_service(agent_key, service_name)

        # 移除生命周期数据
        self.lifecycle_manager.remove_service(agent_key, service_name)

        logger.info(f"Service {service_name} removed from agent {agent_key}")

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
        agent_key = agent_id or self.client_manager.main_client_id
        return self.registry.has_service(agent_key, service_name)

    def _create_standalone_mcp_config(self, config_manager):
        """
        创建独立的MCP配置对象

        Args:
            config_manager: 独立配置管理器

        Returns:
            兼容的MCP配置对象
        """
        class StandaloneMCPConfigAdapter:
            """独立配置适配器 - 兼容MCPConfig接口"""

            def __init__(self, config_manager):
                self.config_manager = config_manager
                self.json_path = ":memory:"  # 表示内存配置

            def load_config(self):
                """加载配置"""
                return self.config_manager.get_mcp_config()

            def get_service_config(self, name):
                """获取服务配置"""
                return self.config_manager.get_service_config(name)

            def save_config(self, config):
                """保存配置（内存模式下不执行实际保存）"""
                logger.info("Standalone mode: config save skipped (memory-only)")
                return True

            def add_service(self, name, config):
                """添加服务"""
                self.config_manager.add_service_config(name, config)
                return True

            def remove_service(self, name):
                """移除服务"""
                # 在独立模式下，我们可以从运行时配置中移除
                services = self.config_manager.get_all_service_configs()
                if name in services:
                    del services[name]
                    logger.info(f"Removed service '{name}' from standalone config")
                    return True
                return False

        return StandaloneMCPConfigAdapter(config_manager)
