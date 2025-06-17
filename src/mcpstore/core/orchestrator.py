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
from mcpstore.plugins.json_mcp import MCPConfig
from mcpstore.core.models.service import TransportType
from mcpstore.core.session_manager import SessionManager

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
        self.pending_reconnection: Set[str] = set()
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
        self.mcp_config = MCPConfig()

        # 客户端管理器
        self.client_manager = ClientManager()

        # 会话管理器
        self.session_manager = SessionManager()

    async def setup(self):
        """初始化编排器资源（不再做服务注册）"""
        logger.info("Setting up MCP Orchestrator...")
        # 只做必要的资源初始化
        pass

    async def start_monitoring(self):
        """启动后台健康检查和重连监视器"""
        logger.info("Starting monitoring tasks...")

        # 启动心跳监视器
        if self.heartbeat_task is None or self.heartbeat_task.done():
            logger.info(f"Starting heartbeat monitor. Interval: {self.heartbeat_interval.total_seconds()}s")
            self.heartbeat_task = asyncio.create_task(self._heartbeat_loop())

        # 启动重连监视器
        if self.reconnection_task is None or self.reconnection_task.done():
            logger.info(f"Starting reconnection monitor. Interval: {self.reconnection_interval.total_seconds()}s")
            self.reconnection_task = asyncio.create_task(self._reconnection_loop())

    async def _heartbeat_loop(self):
        """后台循环，用于定期健康检查"""
        while True:
            await asyncio.sleep(self.heartbeat_interval.total_seconds())
            await self._check_services_health()

    async def _check_services_health(self):
        """检查所有服务的健康状态"""
        logger.debug("Running periodic health check for all services...")
        for client_id, services in self.registry.sessions.items():
            for name in services:
                try:
                    is_healthy = await self.is_service_healthy(name, client_id)
                    if is_healthy:
                        logger.debug(f"Health check SUCCESS for: {name} (client_id={client_id})")
                        self.registry.update_service_health(client_id, name)
                    else:
                        logger.warning(f"Health check FAILED for {name} (client_id={client_id})")
                        self.pending_reconnection.add(name)
                except Exception as e:
                    logger.warning(f"Health check error for {name} (client_id={client_id}): {e}")
                    self.pending_reconnection.add(name)

    async def _reconnection_loop(self):
        """定期尝试重新连接服务的后台循环"""
        while True:
            await asyncio.sleep(self.reconnection_interval.total_seconds())
            await self._attempt_reconnections()

    async def _attempt_reconnections(self):
        """尝试重新连接所有待重连的服务"""
        if not self.pending_reconnection:
            return  # 如果没有待重连的服务，跳过

        # 创建副本以避免迭代过程中修改集合的问题
        names_to_retry = list(self.pending_reconnection)
        logger.info(f"Attempting to reconnect {len(names_to_retry)} service(s): {names_to_retry}")

        for name in names_to_retry:
            try:
                # 尝试重新连接
                success, message = await self.connect_service(name)
                if success:
                    logger.info(f"Reconnection successful for: {name}")
                    self.pending_reconnection.discard(name)
                else:
                    logger.warning(f"Reconnection attempt failed for {name}: {message}")
                    # 保持name在pending_reconnection中，等待下一个周期
            except Exception as e:
                logger.warning(f"Reconnection attempt failed for {name}: {e}")

    async def connect_service(self, name: str, url: str = None) -> Tuple[bool, str]:
        """
        连接到指定的服务

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

            # 创建新的客户端
            client = Client({"mcpServers": {name: service_config}})

            # 尝试连接
            try:
                await client.list_tools()
                self.clients[name] = client
                logger.info(f"Service {name} connected successfully")
                return True, "Connected successfully"
            except Exception as e:
                logger.error(f"Failed to connect to service {name}: {e}")
                return False, str(e)

        except Exception as e:
            logger.error(f"Failed to connect service {name}: {e}")
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
        检查服务是否健康
        
        Args:
            name: 服务名
            client_id: 可选的客户端ID，用于多客户端环境
            
        Returns:
            bool: 服务是否健康
        """
        try:
            # 获取服务配置
            service_config = self.mcp_config.get_service_config(name)
            if not service_config:
                logger.warning(f"Service configuration not found for {name}")
                return False
            
            # 创建新的客户端实例
            client = Client({"mcpServers": {name: service_config}})
            
            try:
                # 使用超时控制的异步上下文管理器
                async with asyncio.timeout(self.http_timeout):
                    async with client:
                        await client.ping()
                        return True
            except asyncio.TimeoutError:
                logger.warning(f"Health check timeout for {name} (client_id={client_id})")
                return False
            except Exception as e:
                logger.warning(f"Health check failed for {name} (client_id={client_id}): {e}")
                return False
            finally:
                # 确保客户端被正确关闭
                try:
                    await client.close()
                except Exception:
                    pass  # 忽略关闭时的错误
                    
        except Exception as e:
            logger.warning(f"Health check failed for {name} (client_id={client_id}): {e}")
            return False

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

    async def execute_tool(
        self,
        service_name: str,
        tool_name: str,
        parameters: Dict[str, Any],
        agent_id: Optional[str] = None
    ) -> Any:
        """执行工具"""
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
                        # 创建新的客户端实例
                        client = Client({"mcpServers": {service_name: service_config}})
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
                        # 创建新的客户端实例
                        client = Client({"mcpServers": {service_name: service_config}})
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

        # 停止监控任务
        if self.heartbeat_task and not self.heartbeat_task.done():
            self.heartbeat_task.cancel()
            try:
                await self.heartbeat_task
            except asyncio.CancelledError:
                pass

        if self.reconnection_task and not self.reconnection_task.done():
            self.reconnection_task.cancel()
            try:
                await self.reconnection_task
            except asyncio.CancelledError:
                pass

        # 关闭所有客户端连接
        for name, client in self.clients.items():
            try:
                await client.close()
            except Exception as e:
                logger.error(f"Error closing client {name}: {e}")

        self.clients.clear()
        self.pending_reconnection.clear()

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

                # 创建新的客户端实例
                client = Client({"mcpServers": {name: service_config}})
                
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
            
            # 使用健康的配置创建客户端
            client = Client(healthy_config)

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
                        tool_name = tool.name
                        
                        # 确定工具所属的服务
                        if is_single_service:
                            # 单服务情况：所有工具都属于这个服务
                            service_name = healthy_services[0]
                            # 如果工具名称还没有服务前缀，添加前缀
                            if not tool_name.startswith(f"{service_name}_"):
                                tool_name = f"{service_name}_{tool_name}"
                        else:
                            # 多服务情况：根据工具名称前缀判断
                            service_name = None
                            for name in healthy_services:
                                if tool_name.startswith(f"{name}_"):
                                    service_name = name
                                    break
                                    
                            if not service_name:
                                logger.warning(f"Tool {tool_name} does not belong to any service, skipping")
                                continue

                        # 处理参数信息
                        parameters = {}
                        if hasattr(tool, 'inputSchema') and tool.inputSchema:
                            parameters = tool.inputSchema
                        elif hasattr(tool, 'parameters') and tool.parameters:
                            parameters = tool.parameters

                        tool_def = {
                            "type": "function",
                            "function": {
                                "name": tool_name,  # 使用可能被修改过的tool_name
                                "description": tool.description,
                                "parameters": parameters
                            }
                        }
                        all_tools.append((tool_name, tool_def))  # 使用可能被修改过的tool_name

                    # 为每个服务注册其工具
                    for service_name in healthy_services:
                        if is_single_service:
                            service_tools = all_tools
                        else:
                            service_tools = [(name, tool_def) for name, tool_def in all_tools if name.startswith(f"{service_name}_")]
                        logger.info(f"Filtered {len(service_tools)} tools for service {service_name}")
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
