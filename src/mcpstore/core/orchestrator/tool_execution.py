"""
MCPOrchestrator Tool Execution Module
Tool execution module - contains tool execution and processing
"""

import logging
from typing import Dict, Any, Optional

from fastmcp import Client

logger = logging.getLogger(__name__)


# Correct session implementation based on langchain_mcp_adapters source code analysis
# Use built-in reentrant context manager features of FastMCP Client

class ToolExecutionMixin:
    """Tool execution mixin class"""

    async def ensure_persistent_client(self, session, service_name: str):
        """Public API: ensure a persistent FastMCP client is created and cached.

        This is a non-breaking wrapper exposing the previously private
        `_create_persistent_client` method, allowing callers (e.g., context/session)
        to depend on a stable public API.
        """
        return await self._create_persistent_client(session, service_name)

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
            service_name: Service name
            tool_name: Tool name (FastMCP original name)
            arguments: Tool parameters
            agent_id: Agent ID (optional)
            timeout: Timeout in seconds
            progress_handler: Progress handler
            raise_on_error: Whether to raise exception on error
            session_id: Session ID (optional, for session-aware execution)

        Returns:
            FastMCP CallToolResult or extracted data
        """
        from mcpstore.core.registry.tool_resolver import FastMCPToolExecutor

        arguments = arguments or {}
        executor = FastMCPToolExecutor(default_timeout=timeout or 30.0)

        # [SESSION MODE] Use cached FastMCP Client
        if session_id:
            logger.info(f"[SESSION_EXECUTION] Using session mode for tool '{tool_name}' in service '{service_name}'")
            return await self._execute_tool_with_session(
                session_id, service_name, tool_name, arguments, agent_id, 
                executor, timeout, progress_handler, raise_on_error
            )

        # [TRADITIONAL MODE] Maintain original logic, ensure backward compatibility
        logger.debug(f"[TRADITIONAL_EXECUTION] Using traditional mode for tool '{tool_name}' in service '{service_name}'")

        try:
            # 确定 effective_agent_id
            effective_agent_id = agent_id if agent_id else self.client_manager.global_agent_store_id
            
            # [pykv 唯一真相源] 从关系层获取 Agent 的服务列表
            relation_manager = self.registry._relation_manager
            agent_services = await relation_manager.get_agent_services(effective_agent_id)
            
            if not agent_services:
                raise Exception(f"No services found in pykv for agent {effective_agent_id}")
            
            logger.debug(f"[TOOL_EXECUTION] pykv relationship layer service count: {len(agent_services)}")
            
            # 从关系层提取 client_ids
            client_ids = list(set(
                svc.get("client_id") for svc in agent_services if svc.get("client_id")
            ))
            
            if not client_ids:
                raise Exception(f"No client_ids found in pykv relations for agent {effective_agent_id}")
            
            logger.debug(f"[TOOL_EXECUTION] pykv relationship layer client_ids: {client_ids}")

            # 检查服务是否存在于关系层
            service_exists = any(
                svc.get("service_global_name") == service_name or 
                svc.get("service_original_name") == service_name
                for svc in agent_services
            )
            
            if not service_exists:
                raise Exception(f"Service {service_name} not found in pykv relations for agent {effective_agent_id}")
            
            # [pykv 唯一真相源] 从实体层获取服务配置
            service_manager = self.registry._cache_service_manager
            service_entity = await service_manager.get_service(service_name)
            
            if not service_entity:
                raise Exception(f"Service entity not found in pykv: {service_name}")
            
            service_config = service_entity.config
            if not service_config:
                raise Exception(f"Service configuration is empty in pykv: {service_name}")
            
            logger.debug(f"[TOOL_EXECUTION] Getting service config from pykv entity layer: {service_name}")

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

                # 预设为用户提供的原始名称（应为 FastMCP 原生方法名）
                effective_tool_name = tool_name

                if not any(t.name == tool_name for t in tools):
                    available = [t.name for t in tools]
                    logger.warning(f"[FASTMCP_DEBUG] not_found tool='{tool_name}' in service='{service_name}'")
                    logger.warning(f"[FASTMCP_DEBUG] available={available}")

                    # 一次性自修复：若传入名称被意外加了前缀，尝试以可用列表为准做最长后缀匹配
                    fallback = None
                    for cand in available:
                        if effective_tool_name.endswith(cand):
                            fallback = cand
                            break

                    if fallback and any(t.name == fallback for t in tools):
                        logger.warning(f"[FASTMCP_DEBUG] self_repair tool_name: '{tool_name}' -> '{fallback}'")
                        effective_tool_name = fallback
                    else:
                        raise Exception(f"Tool {tool_name} not found in service {service_name}. Available: {available}")

                # 使用 FastMCP 标准执行器执行工具
                result = await executor.execute_tool(
                    client=client,
                    tool_name=effective_tool_name,
                    arguments=arguments,
                    timeout=timeout,
                    progress_handler=progress_handler,
                    raise_on_error=raise_on_error
                )

                # 返回 FastMCP 客户端的 CallToolResult（与官方保持一致）
                logger.info(f"[FASTMCP] call ok tool='{effective_tool_name}' service='{service_name}'")
                return result

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
            # Use session_id to get/create named session (priority), otherwise fallback to default session
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

            # Get or create persistent FastMCP Client (refer to langchain_mcp_adapters design)
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
            
            # Use persistent connection to execute tool directly (avoid state loss from closing connection on each async with)
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
                # 
                #        ()
                fallback = None
                for cand in available_tools:
                    if tool_name.endswith(cand):
                        fallback = cand
                        break
                if fallback and any(t.name == fallback for t in tools):
                    logger.warning(f"[SESSION_EXECUTION] self_repair tool_name: '{tool_name}' -> '{fallback}'")
                    #     
                    result = await executor.execute_tool(
                        client=client,
                        tool_name=fallback,
                        arguments=arguments,
                        timeout=timeout,
                        progress_handler=progress_handler,
                        raise_on_error=raise_on_error
                    )
                    logger.info(f"[SESSION_EXECUTION] call ok (repaired) tool='{fallback}' service='{service_name}'")
                    return result

                logger.warning(f"[SESSION_EXECUTION] Tool '{tool_name}' not found in service '{service_name}', available: {available_tools}")
                #     
                #         
                suggestions = []
                try:
                    #        
                    def score(c: str) -> int:
                        s = 0
                        if c in tool_name or tool_name in c:
                            s += 2
                        if c.startswith(tool_name) or tool_name.startswith(c):
                            s += 1
                        return s
                    suggestions = sorted(available_tools, key=lambda c: (-score(c), len(c)))[:3]
                except Exception:
                    suggestions = available_tools[:3]

                raise Exception(
                    f"Tool '{tool_name}' not found in service '{service_name}'. "
                    f"Available: {available_tools}. "
                    f"Try one of: {suggestions} or use bare method name without any prefixes."
                )

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

            # Update session activity time
            session.update_activity()
            
            # Return FastMCP client's CallToolResult (consistent with official implementation)
            logger.info(f"[SESSION_EXECUTION] Tool '{tool_name}' executed successfully in session mode")
            return result
            
        except Exception as e:
            logger.error(f"[SESSION_EXECUTION] Tool execution failed: {e}")
            if raise_on_error:
                raise
            raise Exception(f"Session tool execution failed: {str(e)}")

    async def _create_persistent_client(self, session, service_name: str):
        """
        创建持久的 FastMCP Client 并缓存到会话中
        
        基于 langchain_mcp_adapters 和 FastMCP 源码的正确实现：
        
        核心发现：
        1. FastMCP Client 支持可重入上下文管理器（multiple async with）
        2. 使用引用计数维护连接生命周期
        3. 后台任务管理实际 session 连接
        
        正确的方法：利用 FastMCP Client 的内置机制，不需要自定义 wrapper
        
        [pykv 唯一真相源] 从实体层获取服务配置
        
        Args:
            session: AgentSession 对象
            service_name: 服务名称
            
        Returns:
            Client: 已连接的 FastMCP Client，支持多次复用
        """
        try:
            # [pykv 唯一真相源] 从实体层获取服务配置
            service_manager = self.registry._cache_service_manager
            service_entity = await service_manager.get_service(service_name)
            
            if not service_entity:
                raise Exception(f"Service entity not found in pykv: {service_name}")
            
            service_config = service_entity.config
            if not service_config:
                raise Exception(f"Service configuration is empty in pykv: {service_name}")
            
            # 标准化配置
            normalized_config = self._normalize_service_config(service_config)
            
            # Create FastMCP Client (utilize its reentrant feature)
            client = Client({"mcpServers": {service_name: normalized_config}})
            
            # Start persistent connection (correct usage of FastMCP Client)
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
        await self._start_monitoring()
        logger.info("Monitoring tasks restarted successfully")
