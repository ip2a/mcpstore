"""
MCPStore Tool Operations Module
Implementation of tool-related operations
"""

import logging
from typing import Dict, List, Optional, Any, Union, Literal

from mcpstore.core.models.tool import ToolInfo
from .types import ContextType

logger = logging.getLogger(__name__)

class ToolOperationsMixin:
    """Tool operations mixin class"""

    def _is_tool_available(
        self,
        agent_id: str,
        service_name: str,
        tool_name: str
    ) -> bool:
        """
        检查工具是否在可用集合中（同步版本）
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            tool_name: 工具名称
        
        Returns:
            True 如果工具可用，否则 False
        """
        return self._sync_helper.run_async(
            self._is_tool_available_async(agent_id, service_name, tool_name),
            force_background=True
        )

    async def _is_tool_available_async(
        self,
        agent_id: str,
        service_name: str,
        tool_name: str
    ) -> bool:
        """
        检查工具是否在可用集合中（异步版本）
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            tool_name: 工具名称
        
        Returns:
            True 如果工具可用，否则 False
        """
        try:
            # 获取 ToolSetManager
            tool_set_manager = self._store.tool_set_manager
            
            # 获取工具集状态
            state = await tool_set_manager.get_state_async(agent_id, service_name)
            
            # 如果状态不存在，默认所有工具可用（向后兼容）
            if state is None:
                logger.debug(
                    f"工具集状态不存在，默认可用: agent_id={agent_id}, "
                    f"service={service_name}, tool={tool_name}"
                )
                return True
            
            # 检查工具是否在可用集合中
            # 需要提取工具的原始名称（去除服务前缀）
            original_tool_name = self._extract_original_tool_name(tool_name, service_name)
            is_available = original_tool_name in state.available_tools
            
            logger.debug(
                f"工具可用性检查: agent_id={agent_id}, service={service_name}, "
                f"tool={tool_name}, original={original_tool_name}, available={is_available}"
            )
            
            return is_available
            
        except Exception as e:
            logger.error(
                f"检查工具可用性失败: agent_id={agent_id}, "
                f"service={service_name}, tool={tool_name}, error={e}",
                exc_info=True
            )
            # 出错时默认可用（向后兼容）
            return True

    def _extract_original_tool_name(self, tool_name: str, service_name: str) -> str:
        """
        提取工具的原始名称（去除服务前缀）
        
        Args:
            tool_name: 完整工具名称（可能包含服务前缀）
            service_name: 服务名称
        
        Returns:
            原始工具名称
        """
        # 如果工具名以 "服务名_" 开头，则去除前缀
        prefix = f"{service_name}_"
        if tool_name.startswith(prefix):
            return tool_name[len(prefix):]
        return tool_name

    def list_tools(
        self,
        service_name: Optional[str] = None,
        *,
        filter: Literal["available", "all"] = "available"
    ) -> List[ToolInfo]:
        """
        列出工具（同步版本）
        
        参数说明：
        - filter="available" (默认): 返回当前可用工具集
        - filter="all": 返回原始完整工具集(忽略 add/remove 操作)
        
        Args:
            service_name: 服务名称(可选,None表示所有服务)
            filter: 筛选范围,可选值:
                   - "available": 当前可用工具(默认)
                   - "all": 原始完整工具
        
        Returns:
            工具列表
        
        Examples:
            # 1. 列出当前可用的工具(默认)
            tools = ctx.list_tools()
            
            # 2. 明确指定显示可用工具
            tools = ctx.list_tools(filter="available")
            
            # 3. 列出特定服务的当前可用工具
            weather_tools = ctx.list_tools(service_name="weather")
            
            # 4. 列出原始完整工具集
            all_tools = ctx.list_tools(filter="all")
            
            # 5. 对比当前和原始
            current = ctx.list_tools(service_name="weather", filter="available")
            original = ctx.list_tools(service_name="weather", filter="all")
            removed = set(original) - set(current)
        """
        # Unified waiting strategy: Get consistent snapshot from orchestrator
        logger.info(f"[LIST_TOOLS] start (snapshot) filter={filter}")
        try:
            agent_id = self._agent_id if self._context_type == ContextType.AGENT else None
            snapshot = self._store.orchestrator._sync_helper.run_async(
                self._store.orchestrator.tools_snapshot(agent_id),
                force_background=True
            )
            # Map to ToolInfo
            all_tools = [ToolInfo(**t) for t in snapshot if isinstance(t, dict)]
            
            # 如果指定了服务名称，筛选该服务的工具
            if service_name:
                all_tools = [t for t in all_tools if t.service_name == service_name]
            
            # 如果 filter="all"，直接返回原始列表
            if filter == "all":
                logger.info(f"[LIST_TOOLS] filter=all count={len(all_tools)}")
                return all_tools
            
            # 如果 filter="available"，应用工具集筛选
            # Store 级别不筛选，Agent 级别应用筛选
            if self._context_type != ContextType.AGENT or not agent_id:
                logger.info(f"[LIST_TOOLS] store_mode count={len(all_tools)}")
                return all_tools
            
            # Agent 级别，应用工具集筛选
            filtered_tools = []
            for tool in all_tools:
                if self._is_tool_available(agent_id, tool.service_name, tool.name):
                    filtered_tools.append(tool)
            
            logger.info(
                f"[LIST_TOOLS] filter=available agent={agent_id} "
                f"total={len(all_tools)} available={len(filtered_tools)}"
            )
            return filtered_tools
            
        except Exception as e:
            logger.error(f"[LIST_TOOLS] snapshot error: {e}")
            return []

    async def list_tools_async(
        self,
        service_name: Optional[str] = None,
        *,
        filter: Literal["available", "all"] = "available"
    ) -> List[ToolInfo]:
        """
        列出工具（异步版本）
        
        Args:
            service_name: 服务名称(可选,None表示所有服务)
            filter: 筛选范围,可选值:
                   - "available": 当前可用工具(默认)
                   - "all": 原始完整工具
        
        Returns:
            工具列表
        """
        # Unified to read orchestrator snapshot
        agent_id = self._agent_id if self._context_type == ContextType.AGENT else None
        snapshot = await self._store.orchestrator.tools_snapshot(agent_id)
        all_tools = [ToolInfo(**t) for t in snapshot if isinstance(t, dict)]
        
        # 如果指定了服务名称，筛选该服务的工具
        if service_name:
            all_tools = [t for t in all_tools if t.service_name == service_name]
        
        # 如果 filter="all"，直接返回原始列表
        if filter == "all":
            return all_tools
        
        # 如果 filter="available"，应用工具集筛选
        # Store 级别不筛选，Agent 级别应用筛选
        if self._context_type != ContextType.AGENT or not agent_id:
            return all_tools
        
        # Agent 级别，应用工具集筛选
        filtered_tools = []
        for tool in all_tools:
            if await self._is_tool_available_async(agent_id, tool.service_name, tool.name):
                filtered_tools.append(tool)
        
        return filtered_tools

    def get_tools_with_stats(self) -> Dict[str, Any]:
        """
        Get tool list and statistics (synchronous version)

        Returns:
            Dict: Tool list and statistics
        """
        return self._sync_helper.run_async(self.get_tools_with_stats_async(), force_background=True)

    async def get_tools_with_stats_async(self) -> Dict[str, Any]:
        """
        Get tool list and statistics (asynchronous version)

        Returns:
            Dict: Tool list and statistics
        """
        try:
            tools = await self.list_tools_async()
            
            #  修复：返回完整的工具信息，包括Vue前端需要的所有字段
            tools_data = [
                {
                    "name": tool.name,
                    "description": tool.description,
                    "service_name": tool.service_name,
                    "client_id": tool.client_id,
                    "inputSchema": tool.inputSchema,  # 完整的参数schema
                    "has_schema": tool.inputSchema is not None  # 保持向后兼容
                }
                for tool in tools
            ]

            # 按服务分组统计
            tools_by_service = {}
            for tool in tools:
                service_name = tool.service_name
                if service_name not in tools_by_service:
                    tools_by_service[service_name] = 0
                tools_by_service[service_name] += 1

            #  修复：返回API期望的格式
            return {
                "tools": tools_data,
                "metadata": {
                    "total_tools": len(tools),
                    "services_count": len(tools_by_service),
                    "tools_by_service": tools_by_service
                }
            }
            
        except Exception as e:
            logger.error(f"Failed to get tools with stats: {e}")
            #  修复：错误情况下也返回API期望的格式
            return {
                "tools": [],
                "metadata": {
                    "total_tools": 0,
                    "services_count": 0,
                    "tools_by_service": {},
                    "error": str(e)
                }
            }

    def get_system_stats(self) -> Dict[str, Any]:
        """
        获取系统统计信息（同步版本）

        Returns:
            Dict: 系统统计信息
        """
        return self._sync_helper.run_async(self.get_system_stats_async())

    async def get_system_stats_async(self) -> Dict[str, Any]:
        """
        获取系统统计信息（异步版本）

        Returns:
            Dict: 系统统计信息
        """
        try:
            services = await self.list_services_async()
            tools = await self.list_tools_async()
            
            # 计算统计信息
            stats = {
                "total_services": len(services),
                "total_tools": len(tools),
                "healthy_services": len([s for s in services if getattr(s, "status", None) == "healthy"]),
                "context_type": self._context_type.value,
                "agent_id": self._agent_id,
                "services_by_status": {},
                "tools_by_service": {}
            }
            
            # 按状态分组服务
            for service in services:
                status = getattr(service, "status", "unknown")
                if status not in stats["services_by_status"]:
                    stats["services_by_status"][status] = 0
                stats["services_by_status"][status] += 1
            
            # 按服务分组工具
            for tool in tools:
                service_name = tool.service_name
                if service_name not in stats["tools_by_service"]:
                    stats["tools_by_service"][service_name] = 0
                stats["tools_by_service"][service_name] += 1
            
            return stats
            
        except Exception as e:
            logger.error(f"Failed to get system stats: {e}")
            return {
                "total_services": 0,
                "total_tools": 0,
                "healthy_services": 0,
                "context_type": self._context_type.value,
                "agent_id": self._agent_id,
                "services_by_status": {},
                "tools_by_service": {},
                "error": str(e)
            }

    def batch_add_services(self, services: List[Union[str, Dict[str, Any]]]) -> Dict[str, Any]:
        """
        批量添加服务（同步版本）

        Args:
            services: 服务列表

        Returns:
            Dict: 批量添加结果
        """
        return self._sync_helper.run_async(self.batch_add_services_async(services))

    async def batch_add_services_async(self, services: List[Union[str, Dict[str, Any]]]) -> Dict[str, Any]:
        """
        批量添加服务（异步版本）

        Args:
            services: 服务列表

        Returns:
            Dict: 批量添加结果
        """
        try:
            if not services:
                return {
                    "success": False,
                    "message": "No services provided",
                    "added_services": [],
                    "failed_services": [],
                    "total_added": 0
                }
            
            # 使用现有的 add_service_async 方法
            result = await self.add_service_async(services)
            
            # 获取添加后的服务列表
            current_services = await self.list_services_async()
            service_names = [getattr(s, "name", "unknown") for s in current_services]
            
            return {
                "success": True,
                "message": f"Batch operation completed",
                "added_services": service_names,
                "failed_services": [],
                "total_added": len(service_names)
            }
            
        except Exception as e:
            logger.error(f"Batch add services failed: {e}")
            return {
                "success": False,
                "message": str(e),
                "added_services": [],
                "failed_services": services if isinstance(services, list) else [str(services)],
                "total_added": 0
            }

    def call_tool(self, tool_name: str, args: Union[Dict[str, Any], str] = None, return_extracted: bool = False, **kwargs) -> Any:
        """
        调用工具（同步版本），支持 store/agent 上下文

        用户友好的工具调用接口，支持以下工具名称格式：
        - 直接工具名: "get_weather"
        - 服务前缀（单下划线）: "weather_get_weather"
        注意：不再支持双下划线格式 "service__tool"；如使用将抛出错误并提示迁移方案

        Args:
            tool_name: 工具名称（支持多种格式）
            args: 工具参数（字典或JSON字符串）
            **kwargs: 额外参数（timeout, progress_handler等）

        Returns:
            Any: 工具执行结果
            - 单个内容块：直接返回字符串/数据
            - 多个内容块：返回列表
        """
        # Use background event loop to preserve persistent FastMCP clients across sync calls
        # Especially critical in auto-session mode to avoid per-call asyncio.run() closing loops
        return self._sync_helper.run_async(self.call_tool_async(tool_name, args, return_extracted=return_extracted, **kwargs), force_background=True)

    def use_tool(self, tool_name: str, args: Union[Dict[str, Any], str] = None, return_extracted: bool = False, **kwargs) -> Any:
        """
        使用工具（同步版本）- 向后兼容别名

        注意：此方法是 call_tool 的别名，保持向后兼容性。
        推荐使用 call_tool 方法，与 FastMCP 命名保持一致。
        """
        return self.call_tool(tool_name, args, return_extracted=return_extracted, **kwargs)

    async def call_tool_async(self, tool_name: str, args: Dict[str, Any] = None, return_extracted: bool = False, **kwargs) -> Any:
        """
        调用工具（异步版本），支持 store/agent 上下文

        Args:
            tool_name: 工具名称（支持多种格式）
            args: 工具参数
            **kwargs: 额外参数（timeout, progress_handler等）

        Returns:
            Any: 工具执行结果（FastMCP 标准格式）
        """
        args = args or {}

        # 🎯 隐式会话路由：在 with_session 作用域内且未显式指定 session_id 时优先走当前激活会话
        if getattr(self, '_active_session', None) is not None and 'session_id' not in kwargs:
            try:
                logger.debug(f"[IMPLICIT_SESSION] Routing tool '{tool_name}' to active session '{self._active_session.session_id}'")
            except Exception:
                logger.debug(f"[IMPLICIT_SESSION] Routing tool '{tool_name}' to active session")
            # Avoid duplicate session_id when delegating to Session API
            kwargs.pop('session_id', None)
            return await self._active_session.use_tool_async(tool_name, args, return_extracted=return_extracted, **kwargs)

        # 🎯 自动会话路由：仅当启用了自动会话且未显式指定 session_id 时才路由
        if getattr(self, '_auto_session_enabled', False) and 'session_id' not in kwargs:
            logger.debug(f"[AUTO_SESSION] Routing tool '{tool_name}' to auto session (no explicit session_id)")
            return await self._use_tool_with_session_async(tool_name, args, return_extracted=return_extracted, **kwargs)
        elif getattr(self, '_auto_session_enabled', False) and 'session_id' in kwargs:
            logger.debug("[AUTO_SESSION] Enabled but explicit session_id provided; skip auto routing")

        # 🎯 隐式会话路由：如果 with_session 激活了会话且未显式提供 session_id，则路由到该会话
        active_session = getattr(self, '_active_session', None)
        if active_session is not None and getattr(active_session, 'is_active', False) and 'session_id' not in kwargs:
            logger.debug(f"[ACTIVE_SESSION] Routing tool '{tool_name}' to active session '{active_session.session_id}'")
            kwargs.pop('session_id', None)
            return await active_session.use_tool_async(tool_name, args, return_extracted=return_extracted, **kwargs)

        # 获取可用工具列表用于智能解析
        available_tools = []
        try:
            if self._context_type == ContextType.STORE:
                tools = await self._store.list_tools()
            else:
                tools = await self._store.list_tools(self._agent_id, agent_mode=True)

            # 构建工具信息，包含显示名称和原始名称
            for tool in tools:
                # Agent模式：需要转换服务名称为本地名称
                if self._context_type == ContextType.AGENT and self._agent_id:
                    #  透明代理：将全局服务名转换为本地服务名
                    local_service_name = self._get_local_service_name_from_global(tool.service_name)
                    if local_service_name:
                        # 构建本地工具名称
                        local_tool_name = self._convert_tool_name_to_local(tool.name, tool.service_name, local_service_name)
                        display_name = local_tool_name
                        service_name = local_service_name
                    else:
                        # 如果无法映射，使用原始名称
                        display_name = tool.name
                        service_name = tool.service_name
                else:
                    display_name = tool.name
                    service_name = tool.service_name

                original_name = self._extract_original_tool_name(display_name, service_name)

                available_tools.append({
                    "name": display_name,           # 显示名称（Agent模式下使用本地名称）
                    "original_name": original_name, # 原始名称
                    "service_name": service_name,   # 服务名称（Agent模式下使用本地名称）
                    "global_tool_name": tool.name,  # 保存全局工具名称用于实际调用
                    "global_service_name": tool.service_name  # 保存全局服务名称
                })

            logger.debug(f"Available tools for resolution: {len(available_tools)}")
        except Exception as e:
            logger.warning(f"Failed to get available tools for resolution: {e}")

        # [NEW] Use new intelligent user-friendly resolver
        from mcpstore.core.registry.tool_resolver import ToolNameResolver

        # 检测是否为多服务场景（从已获取的工具列表推导，避免同步→异步桥导致的30s超时）
        derived_services = sorted({
            t.get("service_name") for t in available_tools
            if isinstance(t, dict) and t.get("service_name")
        })

        # 极简兜底：若当前无法从工具列表推导服务（例如工具缓存暂空），
        # 则从 Registry 的同步缓存读取服务名，避免跨异步边界
        if not derived_services:
            try:
                if self._context_type == ContextType.STORE:
                    agent_id = self._store.client_manager.global_agent_store_id
                    cached_services = self._store.registry.get_all_service_names(agent_id)
                    derived_services = sorted(set(cached_services or []))
                else:
                    # Agent 模式：需要将全局服务名映射回本地服务名
                    global_names = self._store.registry.get_agent_services(self._agent_id)
                    local_names = set()
                    for g in (global_names or []):
                        mapping = self._store.registry.get_agent_service_from_global_name(g)
                        if mapping and mapping[0] == self._agent_id:
                            local_names.add(mapping[1])
                    derived_services = sorted(local_names)
                logger.debug(f"[RESOLVE_FALLBACK] derived_services from registry cache: {len(derived_services)}")
            except Exception as e:
                logger.debug(f"[RESOLVE_FALLBACK] failed to derive services from cache: {e}")

        is_multi_server = len(derived_services) > 1

        resolver = ToolNameResolver(
            available_services=derived_services,
            is_multi_server=is_multi_server
        )

        try:
            # 🎯 一站式解析：用户输入 → FastMCP标准格式
            fastmcp_tool_name, resolution = resolver.resolve_and_format_for_fastmcp(tool_name, available_tools)

            logger.info(f"[SMART_RESOLVE] input='{tool_name}' fastmcp='{fastmcp_tool_name}' service='{resolution.service_name}' method='{resolution.resolution_method}'")

        except ValueError as e:
            # LLM-readable error: tool name resolution failed, return structured error for model understanding
            return {
                "content": [{
                    "type": "text",
                    "text": f"[LLM Hint] Tool name resolution failed: {str(e)}. Please check the tool name or add service prefix, e.g. service_tool."
                }],
                "is_error": True
            }

        # 🎯 工具可用性拦截：Agent 模式下检查工具是否可用
        if self._context_type == ContextType.AGENT and self._agent_id:
            # 提取原始工具名称（去除服务前缀）
            original_tool_name = self._extract_original_tool_name(fastmcp_tool_name, resolution.service_name)
            
            # 检查工具是否可用
            is_available = await self._is_tool_available_async(
                self._agent_id,
                resolution.service_name,
                original_tool_name
            )
            
            if not is_available:
                # 工具不可用，抛出异常
                from mcpstore.core.exceptions import ToolNotAvailableError
                
                logger.warning(
                    f"[TOOL_INTERCEPT] 工具不可用: agent_id={self._agent_id}, "
                    f"service={resolution.service_name}, tool={original_tool_name}"
                )
                
                raise ToolNotAvailableError(
                    tool_name=original_tool_name,
                    service_name=resolution.service_name,
                    agent_id=self._agent_id
                )
            
            logger.debug(
                f"[TOOL_INTERCEPT] 工具可用性检查通过: agent_id={self._agent_id}, "
                f"service={resolution.service_name}, tool={original_tool_name}"
            )
        
        # 构造标准化的工具执行请求
        from mcpstore.core.models.tool import ToolExecutionRequest

        if self._context_type == ContextType.STORE:
            logger.info(f"[STORE] call tool='{tool_name}' fastmcp='{fastmcp_tool_name}' service='{resolution.service_name}'")
            request = ToolExecutionRequest(
                tool_name=fastmcp_tool_name,  # [FASTMCP] Use FastMCP standard format
                service_name=resolution.service_name,
                args=args,
                **kwargs
            )
        else:
            # Agent mode: Transparent proxy - map local service name to global service name
            global_service_name = await self._map_agent_tool_to_global_service(resolution.service_name, fastmcp_tool_name)

            logger.info(f"[AGENT:{self._agent_id}] call tool='{tool_name}' fastmcp='{fastmcp_tool_name}' service_local='{resolution.service_name}' service_global='{global_service_name}'")
            request = ToolExecutionRequest(
                tool_name=fastmcp_tool_name,  # [FASTMCP] Use FastMCP standard format
                service_name=global_service_name,  # Use global service name
                args=args,
                agent_id=self._store.client_manager.global_agent_store_id,  # Use global Agent ID
                **kwargs
            )

        response = await self._store.process_tool_request(request)

        # Convert execution errors to LLM-readable format to avoid code interruption
        if hasattr(response, 'success') and not response.success:
            msg = getattr(response, 'error', 'Tool execution failed')
            return {
                "content": [{
                    "type": "text",
                    "text": f"[LLM Hint] Tool invocation failed: {msg}"
                }],
                "is_error": True
            }

        if return_extracted:
            try:
                from mcpstore.core.registry.tool_resolver import FastMCPToolExecutor
                executor = FastMCPToolExecutor()
                return executor.extract_result_data(response.result)
            except Exception:
                # 兜底：无法提取则直接返回原结果
                return getattr(response, 'result', None)
        else:
            # 默认返回 FastMCP 的 CallToolResult（或等价对象）
            return getattr(response, 'result', None)

    async def use_tool_async(self, tool_name: str, args: Dict[str, Any] = None, **kwargs) -> Any:
        """
        使用工具（异步版本）- 向后兼容别名

        注意：此方法是 call_tool_async 的别名，保持向后兼容性。
        推荐使用 call_tool_async 方法，与 FastMCP 命名保持一致。
        """
        return await self.call_tool_async(tool_name, args, **kwargs)

    # ===  新增：Agent 工具调用透明代理方法 ===

    async def _map_agent_tool_to_global_service(self, local_service_name: str, tool_name: str) -> str:
        """
        将 Agent 的本地服务名映射到全局服务名

        Args:
            local_service_name: Agent 中的本地服务名
            tool_name: 工具名称

        Returns:
            str: 全局服务名
        """
        try:
            # 1. 检查是否为 Agent 服务
            if self._agent_id and local_service_name:
                # 尝试从映射关系中获取全局名称
                global_name = self._store.registry.get_global_name_from_agent_service(self._agent_id, local_service_name)
                if global_name:
                    logger.debug(f"[TOOL_PROXY] map local='{local_service_name}' -> global='{global_name}'")
                    return global_name

            # 2. 如果映射失败，检查是否已经是全局名称
            from .agent_service_mapper import AgentServiceMapper
            if AgentServiceMapper.is_any_agent_service(local_service_name):
                logger.debug(f"[TOOL_PROXY] already_global name='{local_service_name}'")
                return local_service_name

            # 3. 如果都不是，可能是 Store 原生服务，直接返回
            logger.debug(f"[TOOL_PROXY] store_native name='{local_service_name}'")
            return local_service_name

        except Exception as e:
            logger.error(f"[TOOL_PROXY] map_error error={e}")
            # 出错时返回原始名称
            return local_service_name

    async def _get_agent_tools_view(self) -> List[ToolInfo]:
        """
        获取 Agent 的工具视图（本地名称）

        透明代理（方案A）：基于映射从 global_agent_store 的缓存派生工具列表，
        不依赖 Agent 命名空间的 sessions/tool_cache。
        """
        try:
            agent_tools: List[ToolInfo] = []
            agent_id = self._agent_id
            global_agent_id = self._store.client_manager.global_agent_store_id

            # 1) 通过映射获取该 Agent 的全局服务名集合
            global_service_names = self._store.registry.get_agent_services(agent_id)
            if not global_service_names:
                logger.info(f"[AGENT_TOOLS] view agent='{agent_id}' count=0 (no mapped services)")
                return agent_tools

            # 2) 遍历映射的全局服务，读取其工具并转换为本地名称
            for global_service_name in global_service_names:
                mapping = self._store.registry.get_agent_service_from_global_name(global_service_name)
                if not mapping:
                    continue
                mapped_agent, local_service_name = mapping
                if mapped_agent != agent_id:
                    continue

                try:
                    # 获取该服务的工具名列表（从全局命名空间）
                    service_tool_names = self._store.registry.get_tools_for_service(
                        global_agent_id,
                        global_service_name
                    )

                    for tool_name in service_tool_names:
                        try:
                            tool_info = self._store.registry.get_tool_info(global_agent_id, tool_name)
                            if not tool_info:
                                logger.warning(f"[AGENT_TOOLS] tool_info_missing name='{tool_name}'")
                                continue

                            # 转换工具名为本地名称
                            local_tool_name = self._convert_tool_name_to_local(tool_name, global_service_name, local_service_name)

                            # 创建本地工具视图（client_id 使用全局命名空间）
                            local_tool = ToolInfo(
                                name=local_tool_name,
                                description=tool_info.get('description', ''),
                                service_name=local_service_name,
                                inputSchema=tool_info.get('inputSchema', {}),
                                client_id=tool_info.get('client_id', '')
                            )
                            agent_tools.append(local_tool)
                            logger.debug(f"[AGENT_TOOLS] add name='{local_tool_name}' service='{local_service_name}'")
                        except Exception as e:
                            logger.error(f"[AGENT_TOOLS] tool_error name='{tool_name}' error={e}")
                            continue
                except Exception as e:
                    logger.error(f"[AGENT_TOOLS] service_tools_error service='{local_service_name}' error={e}")
                    continue

            logger.info(f"[AGENT_TOOLS] view agent='{agent_id}' count={len(agent_tools)}")
            return agent_tools

        except Exception as e:
            logger.error(f"[AGENT_TOOLS] view_error error={e}")
            return []

    def _convert_tool_name_to_local(self, global_tool_name: str, global_service_name: str, local_service_name: str) -> str:
        """
        将全局工具名转换为本地工具名

        Args:
            global_tool_name: 全局工具名
            global_service_name: 全局服务名
            local_service_name: 本地服务名

        Returns:
            str: 本地工具名
        """
        try:
            # If tool name starts with global service name, replace with local service name
            if global_tool_name.startswith(f"{global_service_name}_"):
                tool_suffix = global_tool_name[len(global_service_name) + 1:]
                return f"{local_service_name}_{tool_suffix}"
            else:
                # If format doesn't match, return original tool name
                return global_tool_name

        except Exception as e:
            logger.error(f"[TOOL_NAME_CONVERT] Tool name conversion failed: {e}")
            return global_tool_name

    def _get_local_service_name_from_global(self, global_service_name: str) -> Optional[str]:
        """
        从全局服务名获取本地服务名

        Args:
            global_service_name: 全局服务名

        Returns:
            Optional[str]: 本地服务名，如果不是当前 Agent 的服务则返回 None
        """
        try:
            if not self._agent_id:
                return None

            # Check mapping relationship
            agent_mappings = self._store.registry.agent_to_global_mappings.get(self._agent_id, {})
            for local_name, global_name in agent_mappings.items():
                if global_name == global_service_name:
                    return local_name

            return None

        except Exception as e:
            logger.error(f"[SERVICE_NAME_CONVERT] Service name conversion failed: {e}")
            return None

    # ==================== 工具集管理方法 ====================

    def _resolve_service(
        self,
        service: Union[str, 'ServiceProxy', Literal["_all_services"]]
    ) -> Union[str, List[str]]:
        """
        解析服务参数为服务名称
        
        Args:
            service: 服务标识，支持三种类型：
                    - str: 服务名称
                    - ServiceProxy: 服务代理对象
                    - "_all_services": 保留字符串，表示所有服务
        
        Returns:
            服务名称字符串或服务名称列表（当 service="_all_services" 时）
        
        Raises:
            ValueError: 如果参数类型不支持
            CrossAgentOperationError: 如果尝试跨 Agent 操作
        
        Validates: Requirements 6.9 (跨 Agent 操作防护)
        """
        from mcpstore.core.exceptions import CrossAgentOperationError
        
        # 处理 "_all_services" 保留字符串
        if service == "_all_services":
            # 获取所有服务名称
            services = self.list_services()
            return [getattr(s, "name", str(s)) for s in services]
        
        # 处理 ServiceProxy 对象
        if hasattr(service, "name"):
            # 验证 ServiceProxy 归属（跨 Agent 操作防护）
            if hasattr(service, "is_agent_scoped") and service.is_agent_scoped:
                # 检查 ServiceProxy 是否属于当前 Agent
                service_agent_id = getattr(service, "agent_id", None)
                current_agent_id = self._agent_id
                
                if service_agent_id and current_agent_id and service_agent_id != current_agent_id:
                    raise CrossAgentOperationError(
                        current_agent_id=current_agent_id,
                        service_agent_id=service_agent_id,
                        service_name=service.name,
                        operation="工具集管理"
                    )
                
                logger.debug(f"[TOOL_OPERATIONS] Verified ServiceProxy ownership for '{service.name}'")
            
            return service.name
        
        # 处理字符串
        if isinstance(service, str):
            return service
        
        raise ValueError(f"不支持的服务参数类型: {type(service)}")
    
    async def _verify_data_source_ownership(
        self,
        agent_id: str,
        service_name: str
    ) -> None:
        """
        验证数据源归属
        
        检查工具集状态和服务映射是否存在
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
        
        Raises:
            DataSourceNotFoundError: 数据源不存在
            ServiceMappingError: 服务映射不存在
        
        Validates: Requirements 6.6, 6.10 (数据源归属验证)
        """
        from mcpstore.core.exceptions import DataSourceNotFoundError, ServiceMappingError
        
        tool_set_manager = self._store.tool_set_manager
        
        # 检查工具集状态键是否存在
        state = await tool_set_manager.get_state_async(agent_id, service_name)
        if not state:
            logger.warning(
                f"[TOOL_OPERATIONS] Tool set state not found: "
                f"agent_id={agent_id}, service={service_name}"
            )
            raise DataSourceNotFoundError(
                agent_id=agent_id,
                service_name=service_name,
                data_type="tool_set_state"
            )
        
        # 检查服务映射键是否存在
        mapping = await tool_set_manager.get_service_mapping_async(agent_id, service_name)
        if not mapping:
            logger.warning(
                f"[TOOL_OPERATIONS] Service mapping not found: "
                f"agent_id={agent_id}, service={service_name}"
            )
            raise ServiceMappingError(
                service_name=service_name,
                agent_id=agent_id,
                mapping_type="local_to_global"
            )
        
        logger.debug(
            f"[TOOL_OPERATIONS] Verified data source ownership: "
            f"agent_id={agent_id}, service={service_name}"
        )

    def add_tools(
        self,
        service: Union[str, 'ServiceProxy', Literal["_all_services"]],
        tools: Union[List[str], Literal["_all_tools"]]
    ) -> 'MCPStoreContext':
        """
        添加工具到当前可用集合（同步版本）
        
        操作逻辑：
        - 基于当前状态增量添加
        - 明确指定工具名称
        - 自动去重
        
        Args:
            service: 服务标识，支持三种类型：
                    - str: 服务名称，如 "weather"
                    - ServiceProxy: 服务代理对象，通过 find_service() 获取
                    - "_all_services": 保留字符串，表示所有服务
            
            tools: 工具标识，支持两种类型：
                  - List[str]: 工具名称列表
                    * 具体名称: ["get_current", "get_forecast"]
                  - "_all_tools": 保留字符串，表示所有工具
        
        Returns:
            self (支持链式调用)
        
        Raises:
            ValueError: 如果在 Store 模式下调用
        
        Examples:
            # 1. 使用服务名称 + 工具列表
            ctx.add_tools(service="weather", tools=["get_current", "get_forecast"])
            
            # 2. 使用服务代理对象
            weather_service = ctx.find_service("weather")
            ctx.add_tools(service=weather_service, tools=["get_current"])
            
            # 3. 使用 "_all_tools" 添加所有工具
            ctx.add_tools(service="weather", tools="_all_tools")
            
            # 4. 对所有服务添加工具
            ctx.add_tools(service="_all_services", tools=["get_info"])
            
            # 5. 链式调用
            ctx.add_tools(service="weather", tools=["get_current"]) \\
               .remove_tools(service="weather", tools=["get_history"])
        """
        # 仅在 Agent 模式下生效
        if self._context_type != ContextType.AGENT:
            raise ValueError("add_tools() 仅在 Agent 模式下可用")
        
        return self._sync_helper.run_async(
            self.add_tools_async(service, tools),
            force_background=True
        )

    async def add_tools_async(
        self,
        service: Union[str, 'ServiceProxy', Literal["_all_services"]],
        tools: Union[List[str], Literal["_all_tools"]]
    ) -> 'MCPStoreContext':
        """
        添加工具到当前可用集合（异步版本）
        
        Args:
            service: 服务标识
            tools: 工具标识
        
        Returns:
            self (支持链式调用)
        """
        # 仅在 Agent 模式下生效
        if self._context_type != ContextType.AGENT:
            raise ValueError("add_tools() 仅在 Agent 模式下可用")
        
        # 解析服务参数
        service_names = self._resolve_service(service)
        if isinstance(service_names, str):
            service_names = [service_names]
        
        # 获取 ToolSetManager
        tool_set_manager = self._store.tool_set_manager
        
        # 对每个服务执行添加操作
        errors = []
        for service_name in service_names:
            try:
                # 验证数据源归属
                await self._verify_data_source_ownership(self._agent_id, service_name)
                
                await tool_set_manager.add_tools_async(
                    self._agent_id,
                    service_name,
                    tools
                )
                logger.info(
                    f"添加工具成功: agent_id={self._agent_id}, "
                    f"service={service_name}, tools={tools}"
                )
            except Exception as e:
                logger.error(
                    f"添加工具失败: agent_id={self._agent_id}, "
                    f"service={service_name}, error={e}",
                    exc_info=True
                )
                errors.append((service_name, e))
                # 如果只有一个服务，直接抛出异常
                if len(service_names) == 1:
                    raise
                # 否则继续处理其他服务
        
        # 如果有多个服务且全部失败，抛出第一个异常
        if errors and len(errors) == len(service_names):
            raise errors[0][1]
        
        return self

    def remove_tools(
        self,
        service: Union[str, 'ServiceProxy', Literal["_all_services"]],
        tools: Union[List[str], Literal["_all_tools"]]
    ) -> 'MCPStoreContext':
        """
        从当前可用集合移除工具（同步版本）
        
        操作逻辑：
        - 基于当前状态增量移除
        - 明确指定工具名称
        - 移除不存在的工具不报错
        
        Args:
            service: 服务标识，支持三种类型：
                    - str: 服务名称
                    - ServiceProxy: 服务代理对象
                    - "_all_services": 保留字符串，表示所有服务
            
            tools: 工具标识，支持两种类型：
                  - List[str]: 工具名称列表
                    * 具体名称: ["get_history", "delete_cache"]
                  - "_all_tools": 保留字符串，清空所有工具
        
        Returns:
            self (支持链式调用)
        
        Raises:
            ValueError: 如果在 Store 模式下调用
        
        Examples:
            # 1. 移除具体工具
            ctx.remove_tools(service="weather", tools=["get_history", "delete_cache"])
            
            # 2. 移除多个工具
            ctx.remove_tools(service="database", tools=["delete_table", "drop_table"])
            
            # 3. 清空所有工具
            ctx.remove_tools(service="weather", tools="_all_tools")
            
            # 4. 从所有服务移除工具
            ctx.remove_tools(service="_all_services", tools=["admin_panel"])
            
            # 5. 典型用法: 先清空再添加(实现"只要部分工具")
            ctx.remove_tools(service="weather", tools="_all_tools") \\
               .add_tools(service="weather", tools=["get_current", "get_forecast"])
        """
        # 仅在 Agent 模式下生效
        if self._context_type != ContextType.AGENT:
            raise ValueError("remove_tools() 仅在 Agent 模式下可用")
        
        return self._sync_helper.run_async(
            self.remove_tools_async(service, tools),
            force_background=True
        )

    async def remove_tools_async(
        self,
        service: Union[str, 'ServiceProxy', Literal["_all_services"]],
        tools: Union[List[str], Literal["_all_tools"]]
    ) -> 'MCPStoreContext':
        """
        从当前可用集合移除工具（异步版本）
        
        Args:
            service: 服务标识
            tools: 工具标识
        
        Returns:
            self (支持链式调用)
        """
        # 仅在 Agent 模式下生效
        if self._context_type != ContextType.AGENT:
            raise ValueError("remove_tools() 仅在 Agent 模式下可用")
        
        # 解析服务参数
        service_names = self._resolve_service(service)
        if isinstance(service_names, str):
            service_names = [service_names]
        
        # 获取 ToolSetManager
        tool_set_manager = self._store.tool_set_manager
        
        # 对每个服务执行移除操作
        errors = []
        for service_name in service_names:
            try:
                # 验证数据源归属
                await self._verify_data_source_ownership(self._agent_id, service_name)
                
                await tool_set_manager.remove_tools_async(
                    self._agent_id,
                    service_name,
                    tools
                )
                logger.info(
                    f"移除工具成功: agent_id={self._agent_id}, "
                    f"service={service_name}, tools={tools}"
                )
            except Exception as e:
                logger.error(
                    f"移除工具失败: agent_id={self._agent_id}, "
                    f"service={service_name}, error={e}",
                    exc_info=True
                )
                errors.append((service_name, e))
                # 如果只有一个服务，直接抛出异常
                if len(service_names) == 1:
                    raise
                # 否则继续处理其他服务
        
        # 如果有多个服务且全部失败，抛出第一个异常
        if errors and len(errors) == len(service_names):
            raise errors[0][1]
        
        return self

    def reset_tools(
        self,
        service: Union[str, 'ServiceProxy', Literal["_all_services"]]
    ) -> 'MCPStoreContext':
        """
        重置服务的工具集为默认状态（所有工具）（同步版本）
        
        操作逻辑：
        - 恢复到服务初始化时的状态
        - 等同于 add_tools(service, "_all_tools")
        
        Args:
            service: 服务标识，支持三种类型：
                    - str: 服务名称
                    - ServiceProxy: 服务代理对象
                    - "_all_services": 保留字符串，重置所有服务
        
        Returns:
            self (支持链式调用)
        
        Raises:
            ValueError: 如果在 Store 模式下调用
        
        Examples:
            # 1. 重置单个服务
            ctx.reset_tools(service="weather")
            
            # 2. 使用服务代理
            weather_service = ctx.find_service("weather")
            ctx.reset_tools(service=weather_service)
            
            # 3. 重置所有服务
            ctx.reset_tools(service="_all_services")
            
            # 4. 等价于
            ctx.add_tools(service="weather", tools="_all_tools")
        """
        # 仅在 Agent 模式下生效
        if self._context_type != ContextType.AGENT:
            raise ValueError("reset_tools() 仅在 Agent 模式下可用")
        
        return self._sync_helper.run_async(
            self.reset_tools_async(service),
            force_background=True
        )

    async def reset_tools_async(
        self,
        service: Union[str, 'ServiceProxy', Literal["_all_services"]]
    ) -> 'MCPStoreContext':
        """
        重置服务的工具集为默认状态（异步版本）
        
        Args:
            service: 服务标识
        
        Returns:
            self (支持链式调用)
        """
        # 仅在 Agent 模式下生效
        if self._context_type != ContextType.AGENT:
            raise ValueError("reset_tools() 仅在 Agent 模式下可用")
        
        # 解析服务参数
        service_names = self._resolve_service(service)
        if isinstance(service_names, str):
            service_names = [service_names]
        
        # 获取 ToolSetManager
        tool_set_manager = self._store.tool_set_manager
        
        # 对每个服务执行重置操作
        errors = []
        for service_name in service_names:
            try:
                # 验证数据源归属
                await self._verify_data_source_ownership(self._agent_id, service_name)
                
                await tool_set_manager.reset_tools_async(
                    self._agent_id,
                    service_name
                )
                logger.info(
                    f"重置工具集成功: agent_id={self._agent_id}, "
                    f"service={service_name}"
                )
            except Exception as e:
                logger.error(
                    f"重置工具集失败: agent_id={self._agent_id}, "
                    f"service={service_name}, error={e}",
                    exc_info=True
                )
                errors.append((service_name, e))
                # 如果只有一个服务，直接抛出异常
                if len(service_names) == 1:
                    raise
                # 否则继续处理其他服务
        
        # 如果有多个服务且全部失败，抛出第一个异常
        if errors and len(errors) == len(service_names):
            raise errors[0][1]
        
        return self

    def get_tool_set_info(
        self,
        service: Union[str, 'ServiceProxy']
    ) -> Dict[str, Any]:
        """
        获取服务的工具集信息（同步版本）
        
        Args:
            service: 服务标识(服务名称或服务代理对象)
        
        Returns:
            工具集信息字典
        
        Raises:
            ValueError: 如果在 Store 模式下调用
        
        Examples:
            info = ctx.get_tool_set_info(service="weather")
            # {
            #     "service_name": "weather",
            #     "total_tools": 10,
            #     "available_tools": 5,
            #     "removed_tools": 5,
            #     "last_modified": 1234567890.0,
            #     "operations": [
            #         {"type": "remove", "tools": ["get_history"], "timestamp": ...},
            #         {"type": "add", "tools": ["get_forecast"], "timestamp": ...}
            #     ]
            # }
        """
        # 仅在 Agent 模式下可用
        if self._context_type != ContextType.AGENT:
            raise ValueError("get_tool_set_info() 仅在 Agent 模式下可用")
        
        return self._sync_helper.run_async(
            self.get_tool_set_info_async(service),
            force_background=True
        )

    async def get_tool_set_info_async(
        self,
        service: Union[str, 'ServiceProxy']
    ) -> Dict[str, Any]:
        """
        获取服务的工具集信息（异步版本）
        
        Args:
            service: 服务标识(服务名称或服务代理对象)
        
        Returns:
            工具集信息字典
        """
        # 仅在 Agent 模式下可用
        if self._context_type != ContextType.AGENT:
            raise ValueError("get_tool_set_info() 仅在 Agent 模式下可用")
        
        # 解析服务名称
        if hasattr(service, "name"):
            service_name = service.name
        else:
            service_name = str(service)
        
        try:
            # 获取 ToolSetManager
            tool_set_manager = self._store.tool_set_manager
            
            # 获取工具集状态
            state = await tool_set_manager.get_state_async(self._agent_id, service_name)
            
            # 获取原始工具列表
            all_tools = await tool_set_manager._get_all_tools_async(self._agent_id, service_name)
            
            if state is None:
                # 状态不存在，返回默认信息
                return {
                    "service_name": service_name,
                    "total_tools": len(all_tools),
                    "available_tools": len(all_tools),
                    "removed_tools": 0,
                    "utilization": 1.0,
                    "last_modified": None,
                    "operations": []
                }
            
            # 计算统计信息
            total_tools = len(all_tools)
            available_tools = len(state.available_tools)
            removed_tools = total_tools - available_tools
            utilization = available_tools / total_tools if total_tools > 0 else 0.0
            
            return {
                "service_name": service_name,
                "total_tools": total_tools,
                "available_tools": available_tools,
                "removed_tools": removed_tools,
                "utilization": round(utilization, 2),
                "last_modified": state.updated_at,
                "operations": state.operation_history[-10:]  # 最近10条操作
            }
            
        except Exception as e:
            logger.error(
                f"获取工具集信息失败: agent_id={self._agent_id}, "
                f"service={service_name}, error={e}",
                exc_info=True
            )
            return {
                "service_name": service_name,
                "error": str(e)
            }

    def get_tool_set_summary(self) -> Dict[str, Any]:
        """
        获取工具集摘要（同步版本）
        
        Returns:
            摘要信息字典
        
        Raises:
            ValueError: 如果在 Store 模式下调用
        
        Examples:
            summary = ctx.get_tool_set_summary()
            # {
            #     "total_services": 3,
            #     "services": {
            #         "weather": {
            #             "total_tools": 10,
            #             "available_tools": 5,
            #             "utilization": 0.5
            #         },
            #         "database": {
            #             "total_tools": 20,
            #             "available_tools": 15,
            #             "utilization": 0.75
            #         }
            #     },
            #     "total_available_tools": 20,
            #     "total_original_tools": 30,
            #     "overall_utilization": 0.67
            # }
        """
        # 仅在 Agent 模式下可用
        if self._context_type != ContextType.AGENT:
            raise ValueError("get_tool_set_summary() 仅在 Agent 模式下可用")
        
        return self._sync_helper.run_async(
            self.get_tool_set_summary_async(),
            force_background=True
        )

    async def get_tool_set_summary_async(self) -> Dict[str, Any]:
        """
        获取工具集摘要（异步版本）
        
        Returns:
            摘要信息字典
        """
        # 仅在 Agent 模式下可用
        if self._context_type != ContextType.AGENT:
            raise ValueError("get_tool_set_summary() 仅在 Agent 模式下可用")
        
        try:
            # 获取 ToolSetManager
            tool_set_manager = self._store.tool_set_manager
            
            # 获取 Agent 元数据
            metadata_key = tool_set_manager._get_metadata_key(self._agent_id)
            metadata = await tool_set_manager._kv_store.get(metadata_key)
            
            # 获取所有服务
            services = await self.list_services_async()
            service_names = [getattr(s, "name", str(s)) for s in services]
            
            # 获取每个服务的工具集信息
            services_info = {}
            total_available = 0
            total_original = 0
            
            for service_name in service_names:
                try:
                    info = await self.get_tool_set_info_async(service_name)
                    services_info[service_name] = {
                        "total_tools": info.get("total_tools", 0),
                        "available_tools": info.get("available_tools", 0),
                        "utilization": info.get("utilization", 0.0)
                    }
                    total_available += info.get("available_tools", 0)
                    total_original += info.get("total_tools", 0)
                except Exception as e:
                    logger.warning(
                        f"获取服务工具集信息失败: service={service_name}, error={e}"
                    )
                    services_info[service_name] = {
                        "total_tools": 0,
                        "available_tools": 0,
                        "utilization": 0.0,
                        "error": str(e)
                    }
            
            # 计算总体利用率
            overall_utilization = total_available / total_original if total_original > 0 else 0.0
            
            summary = {
                "agent_id": self._agent_id,
                "total_services": len(service_names),
                "services": services_info,
                "total_available_tools": total_available,
                "total_original_tools": total_original,
                "overall_utilization": round(overall_utilization, 2)
            }
            
            # 如果有元数据，添加额外信息
            if metadata:
                summary["last_operation"] = metadata.get("last_operation")
                summary["statistics"] = metadata.get("statistics")
            
            return summary
            
        except Exception as e:
            logger.error(
                f"获取工具集摘要失败: agent_id={self._agent_id}, error={e}",
                exc_info=True
            )
            return {
                "agent_id": self._agent_id,
                "total_services": 0,
                "services": {},
                "total_available_tools": 0,
                "total_original_tools": 0,
                "overall_utilization": 0.0,
                "error": str(e)
            }
