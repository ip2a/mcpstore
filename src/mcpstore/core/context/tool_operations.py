"""
MCPStore Tool Operations Module
Implementation of tool-related operations
"""

import logging
from typing import Dict, List, Optional, Any, Union

from mcpstore.core.models.tool import ToolInfo
from .types import ContextType

logger = logging.getLogger(__name__)

class ToolOperationsMixin:
    """Tool operations mixin class"""

    def list_tools(self) -> List[ToolInfo]:
        """
        List tools (synchronous version)
        - store context: aggregate tools from all client_ids under global_agent_store
        - agent context: aggregate tools from all client_ids under agent_id

        智能等待机制：
        - 远程服务：最多等待1.5秒
        - 本地服务：最多等待5秒
        - 状态确定后立即返回
        """
        # 🔧 智能等待：先等待INITIALIZING服务就绪
        # 快速路径：如果没有INITIALIZING服务，跳过等待
        if hasattr(self, '_has_initializing_services'):
            from .types import ContextType
            agent_id = self._agent_id if self._context_type == ContextType.AGENT else self._store.client_manager.global_agent_store_id
            has_initializing = self._has_initializing_services(agent_id)

            if has_initializing:
                logger.info(f"[LIST_TOOLS] initializing_detected smart_wait start")
                if hasattr(self, '_wait_for_initializing_services'):
                    self._sync_helper.run_async(self._wait_for_initializing_services(), force_background=True)
                else:
                    logger.warning("[LIST_TOOLS] _wait_for_initializing_services missing")
            else:
                logger.debug("[LIST_TOOLS] no_initializing skip_smart_wait")
        else:
            logger.debug("[LIST_TOOLS] quick_check_unavailable skip_smart_wait")

        # 然后获取工具列表
        logger.info(f"[LIST_TOOLS] start")
        # Avoid forcing background loop to reduce nested loop overhead; set reasonable timeout
        result = self._sync_helper.run_async(self.list_tools_async(), timeout=60.0)
        logger.info(f"[LIST_TOOLS] count={len(result)}")
        if result:
            logger.info(f"[LIST_TOOLS] names={[t.name for t in result]}")
        else:
            logger.warning(f"[LIST_TOOLS] empty=True")
        return result

    async def list_tools_async(self) -> List[ToolInfo]:
        """
        List tools (asynchronous version)
        - store context: aggregate tools from all client_ids under global_agent_store
        - agent context: aggregate tools from all client_ids under agent_id (show local names)
        """
        if self._context_type == ContextType.STORE:
            return await self._store.list_tools()
        else:
            # Agent模式：透明代理 - 获取 Agent 的工具并转换为本地名称
            return await self._get_agent_tools_view()

    def get_tools_with_stats(self) -> Dict[str, Any]:
        """
        获取工具列表及统计信息（同步版本）

        Returns:
            Dict: 包含工具列表和统计信息
        """
        return self._sync_helper.run_async(self.get_tools_with_stats_async())

    async def get_tools_with_stats_async(self) -> Dict[str, Any]:
        """
        获取工具列表及统计信息（异步版本）

        Returns:
            Dict: 包含工具列表和统计信息
        """
        try:
            tools = await self.list_tools_async()
            
            # 🔧 修复：返回完整的工具信息，包括Vue前端需要的所有字段
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

            # 🔧 修复：返回API期望的格式
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
            # 🔧 修复：错误情况下也返回API期望的格式
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

    def call_tool(self, tool_name: str, args: Union[Dict[str, Any], str] = None, **kwargs) -> Any:
        """
        调用工具（同步版本），支持 store/agent 上下文

        用户友好的工具调用接口，支持多种工具名称格式：
        - 直接工具名: "get_weather"
        - 服务前缀: "weather__get_weather"
        - 旧格式: "weather_get_weather"

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
        return self._sync_helper.run_async(self.call_tool_async(tool_name, args, **kwargs), force_background=True)

    def use_tool(self, tool_name: str, args: Union[Dict[str, Any], str] = None, **kwargs) -> Any:
        """
        使用工具（同步版本）- 向后兼容别名

        注意：此方法是 call_tool 的别名，保持向后兼容性。
        推荐使用 call_tool 方法，与 FastMCP 命名保持一致。
        """
        return self.call_tool(tool_name, args, **kwargs)

    async def call_tool_async(self, tool_name: str, args: Dict[str, Any] = None, **kwargs) -> Any:
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
            return await self._active_session.use_tool_async(tool_name, args, **kwargs)

        # 🎯 自动会话路由：仅当启用了自动会话且未显式指定 session_id 时才路由
        if getattr(self, '_auto_session_enabled', False) and 'session_id' not in kwargs:
            logger.debug(f"[AUTO_SESSION] Routing tool '{tool_name}' to auto session (no explicit session_id)")
            return await self._use_tool_with_session_async(tool_name, args, **kwargs)
        elif getattr(self, '_auto_session_enabled', False) and 'session_id' in kwargs:
            logger.debug("[AUTO_SESSION] Enabled but explicit session_id provided; skip auto routing")

        # 🎯 隐式会话路由：如果 with_session 激活了会话且未显式提供 session_id，则路由到该会话
        active_session = getattr(self, '_active_session', None)
        if active_session is not None and getattr(active_session, 'is_active', False) and 'session_id' not in kwargs:
            logger.debug(f"[ACTIVE_SESSION] Routing tool '{tool_name}' to active session '{active_session.session_id}'")
            kwargs.pop('session_id', None)
            return await active_session.use_tool_async(tool_name, args, **kwargs)

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
                    # 🔧 透明代理：将全局服务名转换为本地服务名
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

        # 🚀 使用新的智能用户友好型解析器
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
            raise ValueError(f"智能工具解析失败: {e}")

        # 构造标准化的工具执行请求
        from mcpstore.core.models.tool import ToolExecutionRequest

        if self._context_type == ContextType.STORE:
            logger.info(f"[STORE] call tool='{tool_name}' fastmcp='{fastmcp_tool_name}' service='{resolution.service_name}'")
            request = ToolExecutionRequest(
                tool_name=fastmcp_tool_name,  # 🚀 使用FastMCP标准格式
                service_name=resolution.service_name,
                args=args,
                **kwargs
            )
        else:
            # Agent模式：透明代理 - 将本地服务名映射到全局服务名
            global_service_name = await self._map_agent_tool_to_global_service(resolution.service_name, fastmcp_tool_name)

            logger.info(f"[AGENT:{self._agent_id}] call tool='{tool_name}' fastmcp='{fastmcp_tool_name}' service_local='{resolution.service_name}' service_global='{global_service_name}'")
            request = ToolExecutionRequest(
                tool_name=fastmcp_tool_name,  # 🚀 使用FastMCP标准格式
                service_name=global_service_name,  # 使用全局服务名称
                args=args,
                agent_id=self._store.client_manager.global_agent_store_id,  # 🔧 使用全局 Agent ID
                **kwargs
            )

        return await self._store.process_tool_request(request)

    async def use_tool_async(self, tool_name: str, args: Dict[str, Any] = None, **kwargs) -> Any:
        """
        使用工具（异步版本）- 向后兼容别名

        注意：此方法是 call_tool_async 的别名，保持向后兼容性。
        推荐使用 call_tool_async 方法，与 FastMCP 命名保持一致。
        """
        return await self.call_tool_async(tool_name, args, **kwargs)

    # === 🔧 新增：Agent 工具调用透明代理方法 ===

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
            # 如果工具名以全局服务名开头，替换为本地服务名
            if global_tool_name.startswith(f"{global_service_name}_"):
                tool_suffix = global_tool_name[len(global_service_name) + 1:]
                return f"{local_service_name}_{tool_suffix}"
            else:
                # 如果不符合预期格式，直接返回原工具名
                return global_tool_name

        except Exception as e:
            logger.error(f"❌ [TOOL_NAME_CONVERT] 工具名转换失败: {e}")
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

            # 检查映射关系
            agent_mappings = self._store.registry.agent_to_global_mappings.get(self._agent_id, {})
            for local_name, global_name in agent_mappings.items():
                if global_name == global_service_name:
                    return local_name

            return None

        except Exception as e:
            logger.error(f"❌ [SERVICE_NAME_CONVERT] 服务名转换失败: {e}")
            return None
