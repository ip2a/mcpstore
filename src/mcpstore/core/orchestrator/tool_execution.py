"""
MCPOrchestrator Tool Execution Module
Tool execution module - contains tool execution and processing
"""

import asyncio
import logging
from typing import Dict, List, Any, Optional, Tuple

from fastmcp import Client

logger = logging.getLogger(__name__)


# 基于langchain_mcp_adapters源码分析的正确会话实现
# 使用FastMCP Client的内置可重入上下文管理器特性

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
        raise_on_error: bool = True,
        session_id: Optional[str] = None
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
            session_id: 会话ID（可选，用于会话感知执行）

        Returns:
            FastMCP CallToolResult 或提取的数据
        """
        from mcpstore.core.registry.tool_resolver import FastMCPToolExecutor

        arguments = arguments or {}
        executor = FastMCPToolExecutor(default_timeout=timeout or 30.0)

        # 🎯 会话模式：使用缓存的 FastMCP Client
        if session_id:
            logger.info(f"[SESSION_EXECUTION] Using session mode for tool '{tool_name}' in service '{service_name}'")
            return await self._execute_tool_with_session(
                session_id, service_name, tool_name, arguments, agent_id, 
                executor, timeout, progress_handler, raise_on_error
            )

        # 🎯 传统模式：保持原有逻辑，确保向后兼容
        logger.debug(f"[TRADITIONAL_EXECUTION] Using traditional mode for tool '{tool_name}' in service '{service_name}'")

        try:
            if agent_id:
                # Agent 模式：在指定 Agent 的客户端中查找服务（单源：只依赖缓存）
                client_ids = self.registry.get_agent_clients_from_cache(agent_id)
                if not client_ids:
                    raise Exception(f"No clients found in registry cache for agent {agent_id}")
            else:
                # Store 模式：在 global_agent_store 的客户端中查找服务
                # 🔧 修复：优先从Registry缓存获取，回退到ClientManager持久化文件
                global_agent_id = self.client_manager.global_agent_store_id
                logger.debug(f"🔧 [TOOL_EXECUTION] 查找global_agent_id: {global_agent_id}")

                client_ids = self.registry.get_agent_clients_from_cache(global_agent_id)
                logger.debug(f"🔧 [TOOL_EXECUTION] Registry缓存中的client_ids: {client_ids}")
                logger.debug(f"🔧 [TOOL_EXECUTION] Registry完整agent_clients缓存: {dict(self.registry.agent_clients)}")

                if not client_ids:
                    # 单源模式：不再回退到分片文件
                    logger.warning("Single-source mode: no clients in registry cache for global_agent_store")
                    raise Exception("No clients found in registry cache for global_agent_store")

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

                            # 调试日志：验证工具存在
                            logger.debug(f"[FASTMCP_DEBUG] lookup tool='{tool_name}'")
                            logger.debug(f"[FASTMCP_DEBUG] service='{service_name}' tools:")
                            for i, tool in enumerate(tools):
                                logger.debug(f"   {i+1}. {tool.name}")

                            if not any(t.name == tool_name for t in tools):
                                logger.warning(f"[FASTMCP_DEBUG] not_found tool='{tool_name}' in service='{service_name}'")
                                logger.warning(f"[FASTMCP_DEBUG] available={[t.name for t in tools]}")
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

                            logger.info(f"[FASTMCP] call ok tool='{tool_name}' service='{service_name}'")
                            return extracted_data

                    except Exception as e:
                        logger.error(f"Failed to execute tool in client {client_id}: {e}")
                        if raise_on_error:
                            raise
                        continue

            raise Exception(f"Tool {tool_name} not found in service {service_name}")

        except Exception as e:
            logger.error(f"[FASTMCP] call failed tool='{tool_name}' service='{service_name}' error={e}")
            raise Exception(f"Tool execution failed: {str(e)}")

    async def _execute_tool_with_session(
        self,
        session_id: str,
        service_name: str,
        tool_name: str,
        arguments: Dict[str, Any],
        agent_id: Optional[str],
        executor,
        timeout: Optional[float],
        progress_handler,
        raise_on_error: bool
    ) -> Any:
        """
        会话感知的工具执行模式
        
        使用缓存的 FastMCP Client 执行工具，实现连接复用和状态保持。
        这是解决浏览器会话持久化问题的核心逻辑。
        
        Args:
            session_id: 会话标识
            service_name: 服务名称
            tool_name: 工具名称
            arguments: 工具参数
            agent_id: Agent ID
            executor: FastMCP 执行器
            timeout: 超时时间
            progress_handler: 进度处理器
            raise_on_error: 是否在错误时抛出异常
            
        Returns:
            工具执行结果
        """
        try:
            # 🎯 使用 session_id 获取/创建命名会话（优先），否则回退到默认会话
            effective_agent_id = agent_id or self.client_manager.global_agent_store_id
            session = None
            try:
                if hasattr(self.session_manager, 'get_named_session') and session_id:
                    session = self.session_manager.get_named_session(effective_agent_id, session_id)
                    if not session:
                        logger.info(f"[SESSION_EXECUTION] Named session '{session_id}' not found for agent {effective_agent_id}, creating new named session")
                        if hasattr(self.session_manager, 'create_named_session'):
                            session = self.session_manager.create_named_session(effective_agent_id, session_id)
                if not session:
                    # 回退：使用默认会话
                    session = self.session_manager.get_session(effective_agent_id)
                    if not session:
                        logger.info(f"[SESSION_EXECUTION] Default session not found for agent {effective_agent_id}, creating new session")
                        session = self.session_manager.create_session(effective_agent_id)
            except Exception as e:
                logger.error(f"[SESSION_EXECUTION] Error getting/creating session: {e}")
                # 最后兜底创建一个默认会话
                session = self.session_manager.create_session(effective_agent_id)

            # 🎯 获取或创建持久的 FastMCP Client（参考 langchain_mcp_adapters 设计）
            client = session.services.get(service_name)
            if client is None:
                logger.info(f"[SESSION_EXECUTION] Service '{service_name}' not bound or client is None, creating persistent client")
                client = await self._create_persistent_client(session, service_name)
            else:
                # 如果已有缓存客户端，但未连接，确保连接可用
                try:
                    if hasattr(client, 'is_connected') and not client.is_connected():
                        logger.debug(f"[SESSION_EXECUTION] Cached client for '{service_name}' not connected, calling _connect()")
                        await client._connect()
                except Exception as e:
                    logger.warning(f"[SESSION_EXECUTION] Cached client health check failed for '{service_name}', recreating client: {e}")
                    client = await self._create_persistent_client(session, service_name)

                logger.debug(f"[SESSION_EXECUTION] Reusing cached persistent client for service '{service_name}'")
            
            # 🎯 使用持久连接直接执行工具（避免每次 async with 关闭连接导致状态丢失）
            logger.info(f"[SESSION_EXECUTION] Executing tool '{tool_name}' with persistent client (no async with)")

            import time as _t
            # 确保连接仍然有效
            try:
                if hasattr(client, 'is_connected') and not client.is_connected():
                    t_reconnect0 = _t.perf_counter()
                    await client._connect()
                    t_reconnect1 = _t.perf_counter()
                    logger.debug(f"[TIMING] client._connect() (reconnect): {(t_reconnect1 - t_reconnect0):.3f}s")
            except Exception as e:
                logger.warning(f"[SESSION_EXECUTION] Client reconnect check failed: {e}")

            # 验证工具存在
            t_list0 = _t.perf_counter()
            tools = await client.list_tools()
            t_list1 = _t.perf_counter()
            logger.debug(f"[TIMING] client.list_tools(): {(t_list1 - t_list0):.3f}s")

            if not any(t.name == tool_name for t in tools):
                available_tools = [t.name for t in tools]
                logger.warning(f"[SESSION_EXECUTION] Tool '{tool_name}' not found in service '{service_name}', available: {available_tools}")
                raise Exception(f"Tool {tool_name} not found in service {service_name}")

            # 使用 FastMCP 标准执行器执行工具（不进入 async with，保持连接）
            t_exec0 = _t.perf_counter()
            result = await executor.execute_tool(
                client=client,
                tool_name=tool_name,
                arguments=arguments,
                timeout=timeout,
                progress_handler=progress_handler,
                raise_on_error=raise_on_error
            )
            t_exec1 = _t.perf_counter()
            logger.debug(f"[TIMING] executor.execute_tool(): {(t_exec1 - t_exec0):.3f}s")

            # 5️⃣ 更新会话活跃时间
            session.update_activity()
            
            # 6️⃣ 提取结果数据（按照 FastMCP 标准）
            extracted_data = executor.extract_result_data(result)
            
            logger.info(f"[SESSION_EXECUTION] Tool '{tool_name}' executed successfully in session mode")
            return extracted_data
            
        except Exception as e:
            logger.error(f"[SESSION_EXECUTION] Tool execution failed: {e}")
            if raise_on_error:
                raise
            raise Exception(f"Session tool execution failed: {str(e)}")

    async def _create_persistent_client(self, session, service_name: str):
        """
        创建持久的 FastMCP Client 并缓存到会话中
        
        🎯 基于langchain_mcp_adapters和FastMCP源码的正确实现：
        
        核心发现：
        1. FastMCP Client支持可重入上下文管理器（multiple async with）
        2. 使用引用计数维护连接生命周期
        3. 后台任务管理实际session连接
        
        正确的方法：利用FastMCP Client的内置机制，不需要自定义wrapper
        
        Args:
            session: AgentSession 对象
            service_name: 服务名称
            
        Returns:
            Client: 已连接的FastMCP Client，支持多次复用
        """
        try:
            # 获取服务配置
            service_config = self.mcp_config.get_service_config(service_name)
            if not service_config:
                raise Exception(f"Service configuration not found for {service_name}")
            
            # 标准化配置
            normalized_config = self._normalize_service_config(service_config)
            
            # 🎯 创建 FastMCP Client（利用其可重入特性）
            client = Client({"mcpServers": {service_name: normalized_config}})
            
            # 🎯 启动持久连接（FastMCP Client的正确用法）
            # 注意：我们调用_connect()而不是使用async with，这样连接会保持活跃
            await client._connect()
            
            # 缓存到会话中
            session.add_service(service_name, client)
            
            logger.info(f"[SESSION_EXECUTION] Persistent client created and cached for service '{service_name}'")
            return client
            
        except Exception as e:
            logger.error(f"[SESSION_EXECUTION] Failed to create persistent client for service '{service_name}': {e}")
            raise

# 这些方法已移除 - 使用FastMCP Client的内置连接管理

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
