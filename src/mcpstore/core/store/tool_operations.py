"""
Tool Operations Module
Handles MCPStore tool-related functionality
"""

import logging
import time
from typing import Optional, List, Dict, Any

from mcpstore.core.models.common import ExecutionResponse
from mcpstore.core.models.tool import ToolExecutionRequest, ToolInfo

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
            service_state = self.registry.get_service_state(state_check_agent_id, request.service_name)

            # If service is in unavailable state, return error
            from mcpstore.core.models.service import ServiceConnectionState
            if service_state in [ServiceConnectionState.RECONNECTING, ServiceConnectionState.UNREACHABLE,
                               ServiceConnectionState.DISCONNECTING, ServiceConnectionState.DISCONNECTED]:
                error_msg = f"Service '{request.service_name}' is currently {service_state.value} and unavailable for tool execution"
                logger.warning(error_msg)
                return ExecutionResponse(
                    success=False,
                    result=None,
                    error=error_msg,
                    execution_time=time.time() - start_time,
                    service_name=request.service_name,
                    tool_name=request.tool_name,
                    agent_id=request.agent_id
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

                # Use new detailed recording method
                context._monitoring.record_tool_execution_detailed(
                    tool_name=request.tool_name,
                    service_name=request.service_name,
                    params=request.args,
                    result=result,
                    error=None,
                    response_time=duration_ms
                )
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

                # Use new detailed recording method
                context._monitoring.record_tool_execution_detailed(
                    tool_name=request.tool_name,
                    service_name=request.service_name,
                    params=request.args,
                    result=None,
                    error=str(e),
                    response_time=duration_ms
                )
            except Exception as monitor_error:
                logger.warning(f"Failed to record failed tool execution: {monitor_error}")

            logger.error(f"Tool execution failed: {e}")
            return ExecutionResponse(
                success=False,
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

    def _get_client_id_for_service(self, agent_id: str, service_name: str) -> str:
        """Get the client_id corresponding to the service"""
        try:
            # 1. Look up from agent_clients mapping
            client_ids = self.registry.get_agent_clients_from_cache(agent_id)
            if not client_ids:
                self.logger.warning(f"No client_ids found for agent {agent_id}")
                return ""

            # 2. Iterate through each client_id to find the client containing this service
            for client_id in client_ids:
                client_config = self.registry.get_client_config_from_cache(client_id) or {}
                if service_name in client_config.get("mcpServers", {}):
                    return client_id

            # 3. If not found, return the first client_id as default value
            if client_ids:
                self.logger.warning(f"Service {service_name} not found in any client config, using first client_id: {client_ids[0]}")
                return client_ids[0]

            return ""
        except Exception as e:
            self.logger.error(f"Error getting client_id for service {service_name}: {e}")
            return ""

    async def list_tools(self, id: Optional[str] = None, agent_mode: bool = False) -> List[ToolInfo]:
        """
        列出工具列表（统一走 orchestrator.tools_snapshot 快照）：
        - Store（id 为空或是 global_agent_store）：返回全局快照
        - Agent（agent_mode=True 且 id 为 agent_id）：返回已投影为本地名称的快照
        其他组合不再支持多路径读取，保持简洁一致。
        """
        try:
            if agent_mode and id:
                snapshot = await self.orchestrator.tools_snapshot(agent_id=id)
            else:
                snapshot = await self.orchestrator.tools_snapshot(agent_id=None)
            return [ToolInfo(**t) for t in snapshot if isinstance(t, dict)]
        except Exception as e:
            self.logger.error(f"[STORE.LIST_TOOLS] snapshot error: {e}")
            return []
