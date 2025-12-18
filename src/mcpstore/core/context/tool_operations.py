"""
MCPStore Tool Operations Module
Implementation of tool-related operations

架构原则：Functional Core, Imperative Shell
- 同步版本 (list_tools): 通过 Async Orchestrated Bridge 运行在统一事件循环
- 异步版本 (list_tools_async): 在现有事件循环中执行
- 纯逻辑核心 (ToolLogicCore): 只做计算，不做 IO
- pykv 是唯一真相数据源，不使用内存快照
"""

import logging
from typing import Dict, List, Optional, Any, Union, Literal

from mcpstore.core.models.tool import ToolInfo
from mcpstore.core.logic.tool_logic import ToolLogicCore
from .types import ContextType

logger = logging.getLogger(__name__)


class ToolOperationsMixin:
    """
    工具操作混入类
    
    遵循 Functional Core, Imperative Shell 架构：
    - 同步方法统一通过 Async Orchestrated Bridge 在后台事件循环运行
    - 异步方法在现有事件循环中执行
    - 所有数据从 pykv 读取，不使用内存快照
    """

    # ==================== 工具可用性检查 ====================

    def _is_tool_available(
        self,
        service_global_name: str,
        tool_name: str
    ) -> bool:
        """
        检查工具是否可用（同步外壳）
        
        通过 Async Orchestrated Bridge 在稳定事件循环中执行异步逻辑。
        
        Args:
            service_global_name: 服务全局名称
            tool_name: 工具名称
        
        Returns:
            True 如果工具可用，否则 False
            
        Raises:
            RuntimeError: 如果服务状态不存在或工具状态不存在
        """
        return self._run_async_via_bridge(
            self._is_tool_available_async(service_global_name, tool_name),
            op_name="tool_operations.is_tool_available"
        )

    async def _is_tool_available_async(
        self,
        service_global_name: str,
        tool_name: str
    ) -> bool:
        """
        检查工具是否可用（异步外壳）
        
        从 pykv 状态层读取数据，使用纯逻辑核心进行计算。
        
        Args:
            service_global_name: 服务全局名称
            tool_name: 工具名称
        
        Returns:
            True 如果工具可用，否则 False
            
        Raises:
            RuntimeError: 如果服务状态不存在或工具状态不存在
        """
        # 从 pykv 状态层读取服务状态
        state_manager = self._store.registry._cache_state_manager
        service_status = await state_manager.get_service_status(service_global_name)
        
        # 使用纯逻辑核心进行计算
        # 将 ServiceStatus 对象转换为字典
        status_dict = service_status.to_dict() if service_status else None
        
        is_available = ToolLogicCore.check_tool_availability(
            service_global_name,
            tool_name,
            status_dict
        )
        
        logger.debug(
            f"工具可用性检查: service={service_global_name}, "
            f"tool={tool_name}, available={is_available}"
        )
        
        return is_available

    def _extract_original_tool_name(self, tool_name: str, service_name: str) -> str:
        """
        提取工具的原始名称（去除服务前缀）
        
        委托给纯逻辑核心。
        """
        return ToolLogicCore.extract_original_tool_name(tool_name, service_name)

    # ==================== list_tools 双路外壳 ====================

    def list_tools(
        self,
        service_name: Optional[str] = None,
        *,
        filter: Literal["available", "all"] = "available"
    ) -> List[ToolInfo]:
        """
        列出工具（同步外壳）
        
        通过 Async Orchestrated Bridge 在稳定事件循环中执行异步操作。
        遵循 Functional Core, Imperative Shell 架构。
        
        Args:
            service_name: 服务名称(可选,None表示所有服务)
            filter: 筛选范围
                   - "available": 当前可用工具(默认)
                   - "all": 原始完整工具
        
        Returns:
            工具列表
        """
        return self._run_async_via_bridge(
            self.list_tools_async(service_name, filter=filter),
            op_name="tool_operations.list_tools"
        )

    async def list_tools_async(
        self,
        service_name: Optional[str] = None,
        *,
        filter: Literal["available", "all"] = "available"
    ) -> List[ToolInfo]:
        """
        列出工具（异步外壳）
        
        直接从 pykv 读取数据，不使用内存快照。
        遵循 Functional Core, Imperative Shell 架构。
        
        数据读取路径：
        1. 关系层：获取 Agent 的服务列表
        2. 关系层：获取每个服务的工具列表
        3. 实体层：批量获取工具实体
        4. 状态层：获取服务状态（用于可用性过滤）
        
        Args:
            service_name: 服务名称(可选,None表示所有服务)
            filter: 筛选范围
                   - "available": 当前可用工具(默认)
                   - "all": 原始完整工具
        
        Returns:
            工具列表
        """
        logger.info(f"[LIST_TOOLS] start filter={filter} context_type={self._context_type.name}")
        
        # 确定 agent_id
        if self._context_type == ContextType.AGENT:
            agent_id = self._agent_id
        else:
            agent_id = self._store.orchestrator.client_manager.global_agent_store_id
        
        # ==================== 从 pykv 读取数据 ====================
        
        # 获取管理器
        relation_manager = self._store.registry._relation_manager
        tool_entity_manager = self._store.registry._cache_tool_manager
        state_manager = self._store.registry._cache_state_manager
        
        # Step 1: 从关系层获取 Agent 的服务列表
        agent_services = await relation_manager.get_agent_services(agent_id)
        logger.debug(f"[LIST_TOOLS] agent_services count={len(agent_services)}")
        
        if not agent_services:
            logger.info(f"[LIST_TOOLS] no services for agent_id={agent_id}")
            return []
        
        # Step 2: 从关系层获取每个服务的工具列表
        all_tool_global_names: List[str] = []
        service_tool_map: Dict[str, List[str]] = {}  # service_global_name -> [tool_global_names]
        
        for svc in agent_services:
            service_global_name = svc.get("service_global_name")
            if not service_global_name:
                continue
            
            tool_relations = await relation_manager.get_service_tools(service_global_name)
            tool_names = [
                tr.get("tool_global_name")
                for tr in tool_relations
                if tr.get("tool_global_name")
            ]
            
            service_tool_map[service_global_name] = tool_names
            all_tool_global_names.extend(tool_names)
        
        logger.debug(f"[LIST_TOOLS] total tools to fetch={len(all_tool_global_names)}")
        
        if not all_tool_global_names:
            logger.info(f"[LIST_TOOLS] no tools for agent_id={agent_id}")
            return []
        
        # Step 3: 从实体层批量获取工具实体
        tool_entities = await tool_entity_manager.get_many_tools(all_tool_global_names)
        
        # 构建 client_id 映射
        client_id_map: Dict[str, str] = {}
        for svc in agent_services:
            service_global_name = svc.get("service_global_name")
            client_id = svc.get("client_id")
            if service_global_name and client_id:
                client_id_map[service_global_name] = client_id
        
        # ==================== 使用纯逻辑核心构建工具列表 ====================
        
        # 将实体对象转换为字典
        entity_dicts = [
            e.to_dict() if e else None
            for e in tool_entities
        ]
        
        # 构建工具列表
        all_tools: List[ToolInfo] = []
        for i, entity_dict in enumerate(entity_dicts):
            if entity_dict is None:
                continue
            
            service_global_name = entity_dict.get("service_global_name", "")
            service_original_name = entity_dict.get("service_original_name", "")
            client_id = client_id_map.get(service_global_name)
            
            tool_info = ToolInfo(
                name=entity_dict.get("tool_global_name", ""),
                description=entity_dict.get("description", ""),
                service_name=service_original_name,
                client_id=client_id,
                inputSchema=entity_dict.get("input_schema", {})
            )
            all_tools.append(tool_info)
        
        # 按服务名筛选
        if service_name:
            all_tools = [t for t in all_tools if t.service_name == service_name]
        
        # 如果 filter="all"，直接返回
        if filter == "all":
            logger.info(f"[LIST_TOOLS] filter=all count={len(all_tools)}")
            return all_tools
        
        # ==================== filter="available"，从状态层过滤 ====================
        
        # Step 4: 从状态层获取服务状态
        service_status_map: Dict[str, Dict[str, Any]] = {}
        for svc in agent_services:
            service_global_name = svc.get("service_global_name")
            if not service_global_name:
                continue
            
            status = await state_manager.get_service_status(service_global_name)
            if status:
                service_status_map[service_global_name] = status.to_dict()

                # region agent log
                try:
                    import json, time
                    from pathlib import Path
                    log_path = Path("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log")
                    tools = getattr(status, "tools", []) or []
                    log_record = {
                        "sessionId": "debug-session",
                        "runId": "pre-fix",
                        "hypothesisId": "H3",
                        "location": "tool_operations.py:list_tools_async",
                        "message": "service_status_loaded_for_list_tools",
                        "data": {
                            "agent_id": agent_id,
                            "service_global_name": service_global_name,
                            "health_status": getattr(status, "health_status", None),
                            "tools_count": len(tools),
                            "tool_original_names": [
                                getattr(t, "tool_original_name", None) for t in tools
                            ],
                        },
                        "timestamp": int(time.time() * 1000),
                    }
                    log_path.parent.mkdir(parents=True, exist_ok=True)
                    with log_path.open("a", encoding="utf-8") as f:
                        f.write(json.dumps(log_record, ensure_ascii=False) + "\n")
                except Exception:
                    # 调试日志失败不影响主流程
                    pass
                # endregion
        
        # 使用纯逻辑核心过滤工具
        filtered_tools: List[ToolInfo] = []
        for tool in all_tools:
            # 从工具名中提取服务全局名称
            # 工具名格式：{service_global_name}_{tool_original_name}
            # 但这里 tool.name 是 tool_global_name，需要找到对应的 service_global_name
            
            # 遍历 service_tool_map 找到工具所属的服务
            tool_service_global_name = None
            for svc_global, tool_names in service_tool_map.items():
                if tool.name in tool_names:
                    tool_service_global_name = svc_global
                    break
            
            if tool_service_global_name is None:
                # 找不到服务，跳过（不应该发生）
                logger.warning(f"[LIST_TOOLS] 找不到工具所属服务: tool={tool.name}")
                continue
            
            # 获取服务状态
            status_dict = service_status_map.get(tool_service_global_name)

            # region agent log: before_check_tool_availability (H4)
            try:
                import json, time
                from pathlib import Path
                log_path = Path("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log")
                tools_value = (status_dict or {}).get("tools") if isinstance(status_dict, dict) else None
                tools_count = len(tools_value) if isinstance(tools_value, list) else 0
                log_record = {
                    "sessionId": "debug-session",
                    "runId": "pre-fix",
                    "hypothesisId": "H4",
                    "location": "tool_operations.py:list_tools_async",
                    "message": "before_check_tool_availability",
                    "data": {
                        "agent_id": agent_id,
                        "service_global_name": tool_service_global_name,
                        "tool_name": tool.name,
                        "has_status": status_dict is not None,
                        "tools_count_in_status": tools_count,
                    },
                    "timestamp": int(time.time() * 1000),
                }
                log_path.parent.mkdir(parents=True, exist_ok=True)
                with log_path.open("a", encoding="utf-8") as f:
                    f.write(json.dumps(log_record, ensure_ascii=False) + "\n")
            except Exception:
                # 调试日志失败不影响主流程
                pass
            # endregion

            # 使用纯逻辑核心检查可用性
            try:
                is_available = ToolLogicCore.check_tool_availability(
                    tool_service_global_name,
                    tool.name,
                    status_dict
                )
                if is_available:
                    filtered_tools.append(tool)
            except RuntimeError as e:
                # region agent log: check_tool_availability_error (H8)
                try:
                    import json, time
                    from pathlib import Path
                    log_path = Path("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log")
                    tools_value = (status_dict or {}).get("tools") if isinstance(status_dict, dict) else None
                    tools_count = len(tools_value) if isinstance(tools_value, list) else 0
                    log_record = {
                        "sessionId": "debug-session",
                        "runId": "pre-fix",
                        "hypothesisId": "H8",
                        "location": "tool_operations.py:list_tools_async",
                        "message": "check_tool_availability_error",
                        "data": {
                            "agent_id": agent_id,
                            "service_global_name": tool_service_global_name,
                            "tool_name": tool.name,
                            "error": str(e),
                            "has_status": status_dict is not None,
                            "tools_count_in_status": tools_count,
                        },
                        "timestamp": int(time.time() * 1000),
                    }
                    log_path.parent.mkdir(parents=True, exist_ok=True)
                    with log_path.open("a", encoding="utf-8") as f:
                        f.write(json.dumps(log_record, ensure_ascii=False) + "\n")
                except Exception:
                    # 调试日志失败不影响主流程
                    pass
                # endregion

                # 状态/工具不存在，抛出错误
                raise
        
        logger.info(
            f"[LIST_TOOLS] filter=available agent_id={agent_id} "
            f"total={len(all_tools)} available={len(filtered_tools)}"
        )
        return filtered_tools

    def get_tools_with_stats(self) -> Dict[str, Any]:
        """
        Get tool list and statistics (synchronous version)

        Returns:
            Dict: Tool list and statistics
        """
        return self._run_async_via_bridge(
            self.get_tools_with_stats_async(),
            op_name="tool_operations.get_tools_with_stats"
        )

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
        return self._run_async_via_bridge(
            self.get_system_stats_async(),
            op_name="tool_operations.get_system_stats"
        )

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
        return self._run_async_via_bridge(
            self.batch_add_services_async(services),
            op_name="tool_operations.batch_add_services"
        )

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
        return self._run_async_via_bridge(
            self.call_tool_async(tool_name, args, return_extracted=return_extracted, **kwargs),
            op_name="tool_operations.call_tool"
        )

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

        # 工具可用性拦截：Store 和 Agent 模式都检查工具是否可用
        # 获取服务的全局名称
        if self._context_type == ContextType.AGENT and self._agent_id:
            # Agent 模式：需要将本地服务名映射到全局服务名
            service_global_name = await self._map_agent_tool_to_global_service(
                resolution.service_name, fastmcp_tool_name
            )
        else:
            # Store 模式：服务名就是全局名称
            service_global_name = resolution.service_name
        
        # 检查工具是否可用
        is_available = await self._is_tool_available_async(
            service_global_name,
            fastmcp_tool_name
        )
        
        if not is_available:
            # 工具不可用，抛出异常
            from mcpstore.core.exceptions import ToolNotAvailableError
            
            original_tool_name = self._extract_original_tool_name(fastmcp_tool_name, resolution.service_name)
            agent_id = self._agent_id if self._context_type == ContextType.AGENT else "global_agent_store"
            
            logger.warning(
                f"[TOOL_INTERCEPT] 工具不可用: agent_id={agent_id}, "
                f"service_global_name={service_global_name}, tool={original_tool_name}"
            )
            
            raise ToolNotAvailableError(
                tool_name=original_tool_name,
                service_name=resolution.service_name,
                agent_id=agent_id
            )
        
        logger.debug(
            f"[TOOL_INTERCEPT] 工具可用性检查通过: "
            f"service_global_name={service_global_name}, tool={fastmcp_tool_name}"
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
        from mcpstore.core.exceptions import DataSourceNotFoundError
        
        # 获取服务的全局名称
        service_global_name = self._store.registry.get_global_name_from_agent_service(
            agent_id, service_name
        )
        
        if not service_global_name:
            raise DataSourceNotFoundError(
                agent_id=agent_id,
                service_name=service_name,
                data_type="service_mapping"
            )
        
        # 检查服务状态是否存在
        state_manager = self._store.registry._cache_state_manager
        service_status = await state_manager.get_service_status(service_global_name)
        
        if not service_status:
            raise DataSourceNotFoundError(
                agent_id=agent_id,
                service_name=service_name,
                data_type="service_status"
            )
        
        logger.debug(
            f"[TOOL_OPERATIONS] Verified data source ownership: "
            f"agent_id={agent_id}, service={service_name}, "
            f"service_global_name={service_global_name}"
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
        
        return self._run_async_via_bridge(
            self.add_tools_async(service, tools),
            op_name="tool_operations.add_tools"
        )

    async def add_tools_async(
        self,
        service: Union[str, 'ServiceProxy', Literal["_all_services"]],
        tools: Union[List[str], Literal["_all_tools"]]
    ) -> 'MCPStoreContext':
        """
        添加工具到当前可用集合（异步版本）
        
        使用 StateManager 更新工具状态为 "available"。
        
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
        
        # 获取 StateManager
        state_manager = self._store.registry._cache_state_manager
        
        # 对每个服务执行添加操作
        for service_name in service_names:
            # 验证数据源归属
            await self._verify_data_source_ownership(self._agent_id, service_name)
            
            # 获取服务的全局名称
            service_global_name = self._store.registry.get_global_name_from_agent_service(
                self._agent_id, service_name
            )
            
            if not service_global_name:
                raise RuntimeError(
                    f"无法获取服务全局名称: agent_id={self._agent_id}, "
                    f"service_name={service_name}"
                )
            
            # 获取服务状态
            service_status = await state_manager.get_service_status(service_global_name)
            
            if not service_status:
                raise RuntimeError(
                    f"服务状态不存在: service_global_name={service_global_name}"
                )
            
            # 确定要添加的工具列表
            if tools == "_all_tools":
                # 添加所有工具
                tool_names = [t.tool_original_name for t in service_status.tools]
            else:
                tool_names = tools
            
            # 批量设置工具为可用
            await state_manager.batch_set_tools_status(
                service_global_name,
                tool_names,
                "available"
            )
            
            logger.info(
                f"添加工具成功: agent_id={self._agent_id}, "
                f"service={service_name}, tools={tool_names}"
            )
        
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
        
        return self._run_async_via_bridge(
            self.remove_tools_async(service, tools),
            op_name="tool_operations.remove_tools"
        )

    async def remove_tools_async(
        self,
        service: Union[str, 'ServiceProxy', Literal["_all_services"]],
        tools: Union[List[str], Literal["_all_tools"]]
    ) -> 'MCPStoreContext':
        """
        从当前可用集合移除工具（异步版本）
        
        使用 StateManager 更新工具状态为 "unavailable"。
        
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
        
        # 获取 StateManager
        state_manager = self._store.registry._cache_state_manager
        
        # 对每个服务执行移除操作
        for service_name in service_names:
            # 验证数据源归属
            await self._verify_data_source_ownership(self._agent_id, service_name)
            
            # 获取服务的全局名称
            service_global_name = self._store.registry.get_global_name_from_agent_service(
                self._agent_id, service_name
            )
            
            if not service_global_name:
                raise RuntimeError(
                    f"无法获取服务全局名称: agent_id={self._agent_id}, "
                    f"service_name={service_name}"
                )
            
            # 获取服务状态
            service_status = await state_manager.get_service_status(service_global_name)
            
            if not service_status:
                raise RuntimeError(
                    f"服务状态不存在: service_global_name={service_global_name}"
                )
            
            # 确定要移除的工具列表
            if tools == "_all_tools":
                # 移除所有工具
                tool_names = [t.tool_original_name for t in service_status.tools]
            else:
                tool_names = tools
            
            # 批量设置工具为不可用
            await state_manager.batch_set_tools_status(
                service_global_name,
                tool_names,
                "unavailable"
            )
            
            logger.info(
                f"移除工具成功: agent_id={self._agent_id}, "
                f"service={service_name}, tools={tool_names}"
            )
        
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
        
        return self._run_async_via_bridge(
            self.reset_tools_async(service),
            op_name="tool_operations.reset_tools"
        )

    async def reset_tools_async(
        self,
        service: Union[str, 'ServiceProxy', Literal["_all_services"]]
    ) -> 'MCPStoreContext':
        """
        重置服务的工具集为默认状态（异步版本）
        
        将所有工具状态重置为 "available"。
        
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
        
        # 获取 StateManager
        state_manager = self._store.registry._cache_state_manager
        
        # 对每个服务执行重置操作
        for service_name in service_names:
            # 验证数据源归属
            await self._verify_data_source_ownership(self._agent_id, service_name)
            
            # 获取服务的全局名称
            service_global_name = self._store.registry.get_global_name_from_agent_service(
                self._agent_id, service_name
            )
            
            if not service_global_name:
                raise RuntimeError(
                    f"无法获取服务全局名称: agent_id={self._agent_id}, "
                    f"service_name={service_name}"
                )
            
            # 获取服务状态
            service_status = await state_manager.get_service_status(service_global_name)
            
            if not service_status:
                raise RuntimeError(
                    f"服务状态不存在: service_global_name={service_global_name}"
                )
            
            # 获取所有工具名称
            all_tool_names = [t.tool_original_name for t in service_status.tools]
            
            # 批量设置所有工具为可用
            if all_tool_names:
                await state_manager.batch_set_tools_status(
                    service_global_name,
                    all_tool_names,
                    "available"
                )
            
            logger.info(
                f"重置工具集成功: agent_id={self._agent_id}, "
                f"service={service_name}, tools_count={len(all_tool_names)}"
            )
        
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
        
        return self._run_async_via_bridge(
            self.get_tool_set_info_async(service),
            op_name="tool_operations.get_tool_set_info"
        )

    async def get_tool_set_info_async(
        self,
        service: Union[str, 'ServiceProxy']
    ) -> Dict[str, Any]:
        """
        获取服务的工具集信息（异步版本）
        
        使用 StateManager 获取工具状态信息。
        
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
        
        # 获取服务的全局名称
        service_global_name = self._store.registry.get_global_name_from_agent_service(
            self._agent_id, service_name
        )
        
        if not service_global_name:
            raise RuntimeError(
                f"无法获取服务全局名称: agent_id={self._agent_id}, "
                f"service_name={service_name}"
            )
        
        # 获取 StateManager
        state_manager = self._store.registry._cache_state_manager
        
        # 获取服务状态
        service_status = await state_manager.get_service_status(service_global_name)
        
        if not service_status:
            raise RuntimeError(
                f"服务状态不存在: service_global_name={service_global_name}"
            )
        
        # 计算统计信息
        total_tools = len(service_status.tools)
        available_tools = sum(
            1 for t in service_status.tools if t.status == "available"
        )
        unavailable_tools = total_tools - available_tools
        utilization = available_tools / total_tools if total_tools > 0 else 0.0
        
        # 构建工具列表
        tools_info = [
            {
                "name": t.tool_original_name,
                "global_name": t.tool_global_name,
                "status": t.status
            }
            for t in service_status.tools
        ]
        
        return {
            "service_name": service_name,
            "service_global_name": service_global_name,
            "health_status": service_status.health_status,
            "total_tools": total_tools,
            "available_tools": available_tools,
            "unavailable_tools": unavailable_tools,
            "utilization": round(utilization, 2),
            "last_health_check": service_status.last_health_check,
            "tools": tools_info
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
        
        return self._run_async_via_bridge(
            self.get_tool_set_summary_async(),
            op_name="tool_operations.get_tool_set_summary"
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
                    logger.error(
                        f"获取服务工具集信息失败: service={service_name}, error={e}"
                    )
                    raise
            
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
            
            return summary
            
        except Exception as e:
            logger.error(
                f"获取工具集摘要失败: agent_id={self._agent_id}, error={e}",
                exc_info=True
            )
            raise
