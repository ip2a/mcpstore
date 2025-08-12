"""
MCPOrchestrator Tool Execution Module
Tool execution module - contains tool execution and processing
"""

import asyncio
import logging
from typing import Dict, List, Any, Optional, Tuple

from fastmcp import Client

logger = logging.getLogger(__name__)

class ToolExecutionMixin:
    """Tool execution mixin class"""

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
        Execute tool (FastMCP standard)
        Strictly execute tool calls according to FastMCP official standards

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
        from mcpstore.core.registry.tool_resolver import FastMCPToolExecutor

        arguments = arguments or {}
        executor = FastMCPToolExecutor(default_timeout=timeout or 30.0)

        try:
            if agent_id:
                # Agent 模式：在指定 Agent 的客户端中查找服务
                # 🔧 修复：优先从Registry缓存获取，回退到ClientManager持久化文件
                client_ids = self.registry.get_agent_clients_from_cache(agent_id)
                if not client_ids:
                    # 回退到持久化文件
                    client_ids = self.client_manager.get_agent_clients(agent_id)
                    if not client_ids:
                        raise Exception(f"No clients found for agent {agent_id}")
            else:
                # Store 模式：在 global_agent_store 的客户端中查找服务
                # 🔧 修复：优先从Registry缓存获取，回退到ClientManager持久化文件
                global_agent_id = self.client_manager.global_agent_store_id
                logger.debug(f"🔧 [TOOL_EXECUTION] 查找global_agent_id: {global_agent_id}")

                client_ids = self.registry.get_agent_clients_from_cache(global_agent_id)
                logger.debug(f"🔧 [TOOL_EXECUTION] Registry缓存中的client_ids: {client_ids}")
                logger.debug(f"🔧 [TOOL_EXECUTION] Registry完整agent_clients缓存: {dict(self.registry.agent_clients)}")

                if not client_ids:
                    # 回退到持久化文件
                    logger.warning(f"🔧 [TOOL_EXECUTION] Registry缓存为空，回退到持久化文件")
                    client_ids = self.client_manager.get_agent_clients(global_agent_id)
                    logger.debug(f"🔧 [TOOL_EXECUTION] ClientManager文件中的client_ids: {client_ids}")
                    if not client_ids:
                        logger.error(f"🔧 [TOOL_EXECUTION] 持久化文件也为空！")
                        logger.error(f"🔧 [TOOL_EXECUTION] 检查agent_clients.json文件内容")
                        raise Exception("No clients found in global_agent_store")

            # 遍历客户端查找服务
            for client_id in client_ids:
                # 🔧 修复：has_service需要正确的agent_id
                effective_agent_id = agent_id if agent_id else self.client_manager.global_agent_store_id
                if self.registry.has_service(effective_agent_id, service_name):
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

                            # 🔧 调试日志：验证工具存在
                            logger.debug(f"🔍 [FASTMCP_DEBUG] 查找工具: {tool_name}")
                            logger.debug(f"🔍 [FASTMCP_DEBUG] 服务 {service_name} 中的实际工具:")
                            for i, tool in enumerate(tools):
                                logger.debug(f"   {i+1}. {tool.name}")

                            if not any(t.name == tool_name for t in tools):
                                logger.warning(f"🔍 [FASTMCP_DEBUG] 工具 {tool_name} 在服务 {service_name} 中未找到!")
                                logger.warning(f"🔍 [FASTMCP_DEBUG] 可用工具: {[t.name for t in tools]}")
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
