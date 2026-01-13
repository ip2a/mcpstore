"""
Tool Operations Module
Handles MCPStore tool-related functionality
"""

import logging
import time
from typing import Optional, List, Dict, Any

from mcpstore.core.models.common import ExecutionResponse
from mcpstore.core.models.tool import ToolExecutionRequest, ToolInfo
from mcpstore.core.models.tool_result import CallToolFailureResult

logger = logging.getLogger(__name__)


class ToolOperationsMixin:
    """Tool operations Mixin"""

    async def process_tool_request(self, request: ToolExecutionRequest) -> ExecutionResponse:
        """
        Process tool execution request (FastMCP standard)

        Args:
            request: Tool execution request

        Returns:
            ExecutionResponse: Tool execution response
        """
        start_time = time.time()

        try:
            # Validate request parameters
            if not request.tool_name:
                raise ValueError("Tool name cannot be empty")
            if not request.service_name:
                raise ValueError("Service name cannot be empty")

            logger.debug(f"Processing tool request: {request.service_name}::{request.tool_name}")

            # Check service lifecycle state
            # For Agent transparent proxy, global services exist in global_agent_store
            if request.agent_id and "_byagent_" in request.service_name:
                # Agent transparent proxy: global services are in global_agent_store
                state_check_agent_id = self.client_manager.global_agent_store_id
            else:
                # Store mode or normal Agent services
                state_check_agent_id = request.agent_id or self.client_manager.global_agent_store_id

            # Event-driven architecture: get state directly from registry (no longer through lifecycle_manager)
            # 在 async 方法中必须使用 async 版本，避免 AOB 检测到已有事件循环抛出 RuntimeError
            service_state = await self.registry._service_state_service.get_service_state_async(state_check_agent_id, request.service_name)

            # 如果状态不健康，先记录但仍尝试执行，失败时再返回真实错误
            from mcpstore.core.models.service import ServiceConnectionState
            state_warn = service_state in [
                ServiceConnectionState.CIRCUIT_OPEN,
                ServiceConnectionState.HALF_OPEN,
                ServiceConnectionState.DISCONNECTED
            ]
            if state_warn:
                logger.warning(
                    f"Service '{request.service_name}' is in state {service_state.value}, will still attempt execution"
                )

            # Execute tool (using FastMCP standard)
            result = await self.orchestrator.execute_tool_fastmcp(
                service_name=request.service_name,
                tool_name=request.tool_name,
                arguments=request.args,
                agent_id=request.agent_id,
                timeout=request.timeout,
                progress_handler=request.progress_handler,
                raise_on_error=request.raise_on_error,
                session_id=getattr(request, 'session_id', None)  # [NEW] Pass session ID if available
            )

            # [MONITORING] Record successful tool execution
            try:
                duration_ms = (time.time() - start_time) * 1000

                # Get corresponding Context to record monitoring data
                if request.agent_id:
                    context = self.for_agent(request.agent_id)
                else:
                    context = self.for_store()

                if getattr(context, "_monitoring", None):
                    context._monitoring.record_tool_execution_detailed(
                        tool_name=request.tool_name,
                        service_name=request.service_name,
                        params=request.args,
                        result=result,
                        error=None,
                        response_time=duration_ms
                    )
                # 被动反馈：写入健康滑窗
                try:
                    container = getattr(self, "container", None)
                    health_monitor = getattr(container, "health_monitor", None) if container else None
                    if health_monitor:
                        health_monitor.record_passive_feedback(
                            agent_id=state_check_agent_id,
                            service_name=request.service_name,
                            success=True,
                            response_time=duration_ms / 1000.0,
                        )
                except Exception as hf_err:
                    logger.debug(f"[MONITORING] passive feedback (success) failed: {hf_err}")
            except Exception as monitor_error:
                logger.warning(f"Failed to record tool execution: {monitor_error}")

            return ExecutionResponse(
                success=True,
                result=result
            )
        except Exception as e:
            # [MONITORING] Record failed tool execution
            try:
                duration_ms = (time.time() - start_time) * 1000

                # Get corresponding Context to record monitoring data
                if request.agent_id:
                    context = self.for_agent(request.agent_id)
                else:
                    context = self.for_store()

                if getattr(context, "_monitoring", None):
                    context._monitoring.record_tool_execution_detailed(
                        tool_name=request.tool_name,
                        service_name=request.service_name,
                        params=request.args,
                        result=None,
                        error=str(e),
                        response_time=duration_ms
                    )
                # 被动反馈：写入健康滑窗
                try:
                    container = getattr(self, "container", None)
                    health_monitor = getattr(container, "health_monitor", None) if container else None
                    if health_monitor:
                        health_monitor.record_passive_feedback(
                            agent_id=state_check_agent_id,
                            service_name=request.service_name,
                            success=False,
                            response_time=duration_ms / 1000.0,
                        )
                except Exception as hf_err:
                    logger.debug(f"[MONITORING] passive feedback (failure) failed: {hf_err}")
            except Exception as monitor_error:
                logger.warning(f"Failed to record failed tool execution: {monitor_error}")

            logger.error(f"Tool execution failed: {e}")
            failure_result = CallToolFailureResult(str(e)).unwrap()
            return ExecutionResponse(
                success=False,
                result=failure_result,
                error=str(e)
            )

    async def call_tool(self, tool_name: str, args: Dict[str, Any]) -> Any:
        """
        Call tool (generic interface)

        Args:
            tool_name: Tool name, format: service_toolname
            args: Tool parameters

        Returns:
            Any: Tool execution result
        """
        from mcpstore.core.models.tool import ToolExecutionRequest

        # Build request
        request = ToolExecutionRequest(
            tool_name=tool_name,
            args=args
        )

        # Process tool request
        return await self.process_tool_request(request)

    async def use_tool(self, tool_name: str, args: Dict[str, Any]) -> Any:
        """
        Use tool (generic interface) - backward compatibility alias

        Note: This method is an alias for call_tool, maintaining backward compatibility.
        It is recommended to use the call_tool method to remain consistent with FastMCP naming.
        """
        return await self.call_tool(tool_name, args)

    async def _get_client_id_for_service_async(self, agent_id: str, service_name: str) -> str:
        """
        获取服务对应的 client_id
        
        [pykv 唯一真相源] 从 pykv 关系层读取
        """
        # 从 pykv 关系层获取 Agent 的服务列表
        relation_manager = self.registry._relation_manager
        agent_services = await relation_manager.get_agent_services(agent_id)
        
        if not agent_services:
            self.logger.warning(f"No services found in pykv for agent {agent_id}")
            return ""
        
        # 查找指定服务的 client_id
        for svc in agent_services:
            if svc.get("service_global_name") == service_name or svc.get("service_original_name") == service_name:
                client_id = svc.get("client_id")
                if client_id:
                    return client_id
        
        # 如果没找到，返回第一个 client_id 作为默认值
        first_client_id = agent_services[0].get("client_id") if agent_services else ""
        if first_client_id:
            self.logger.warning(f"Service {service_name} not found, using first client_id: {first_client_id}")
        return first_client_id

    async def list_tools(self, id: Optional[str] = None, agent_mode: bool = False) -> List[ToolInfo]:
        """
        列出工具列表（直接从 pykv 读取，不使用快照）
        
        遵循 Functional Core, Imperative Shell 架构：
        - pykv 是唯一真相数据源
        - 不使用内存快照
        
        Args:
            id: Agent ID（可选）
            agent_mode: 是否为 Agent 模式
        
        Returns:
            工具列表
        """
        # 确定 agent_id
        if agent_mode and id:
            agent_id = id
        else:
            agent_id = self.client_manager.global_agent_store_id
        
        # 获取管理器
        relation_manager = self.registry._relation_manager
        tool_entity_manager = self.registry._cache_tool_manager
        
        # Step 1: 从关系层获取 Agent 的服务列表
        agent_services = await relation_manager.get_agent_services(agent_id)
        
        if not agent_services:
            self.logger.debug(f"[STORE.LIST_TOOLS] no services for agent_id={agent_id}")
            return []
        
        # Step 2: 从关系层获取每个服务的工具列表
        all_tool_global_names: List[str] = []
        
        for svc in agent_services:
            service_global_name = svc.get("service_global_name")
            if not service_global_name:
                continue
            
            tool_relations = await relation_manager.get_service_tools(service_global_name)
            for tr in tool_relations:
                tool_global_name = tr.get("tool_global_name")
                if tool_global_name:
                    all_tool_global_names.append(tool_global_name)
        
        if not all_tool_global_names:
            self.logger.debug(f"[STORE.LIST_TOOLS] no tools for agent_id={agent_id}")
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
        
        # Step 4: 构建工具列表
        tools: List[ToolInfo] = []
        for entity in tool_entities:
            if entity is None:
                continue
            
            entity_dict = entity.to_dict() if hasattr(entity, 'to_dict') else entity
            service_global_name = entity_dict.get("service_global_name", "")
            service_original_name = entity_dict.get("service_original_name", "")
            client_id = client_id_map.get(service_global_name)
            
            tool_info = ToolInfo(
                name=entity_dict.get("tool_global_name", ""),
                tool_original_name=entity_dict.get("tool_original_name", ""),
                description=entity_dict.get("description", ""),
                service_name=service_original_name,
                service_original_name=service_original_name,
                service_global_name=service_global_name,
                client_id=client_id,
                inputSchema=entity_dict.get("input_schema", {})
            )
            tools.append(tool_info)
        
        self.logger.debug(f"[STORE.LIST_TOOLS] agent_id={agent_id} tools_count={len(tools)}")
        return tools
