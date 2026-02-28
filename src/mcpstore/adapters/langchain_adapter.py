# src/mcpstore/adapters/langchain_adapter.py
"""
LangChain 适配器模块

将 MCPStore 工具转换为 LangChain 工具格式，支持同步和异步执行。
"""

import json
import logging
from dataclasses import asdict, is_dataclass
from typing import Type, List, TYPE_CHECKING

from langchain_core.tools import Tool, StructuredTool, ToolException
from pydantic import BaseModel

# 导入公共函数
from .common import (
    call_tool_response_helper,
    create_args_schema,
    enhance_description,
    process_tool_args,
)
from ..core.bridge import get_bridge_executor

if TYPE_CHECKING:
    from ..core.context import MCPStoreContext

logger = logging.getLogger(__name__)


class LangChainAdapter:
    """
    MCPStore 与 LangChain 之间的适配器。
    将 mcpstore 的原生对象转换为 LangChain 可直接使用的对象。
    """

    def __init__(self, context: 'MCPStoreContext', response_format: str = "text"):
        self._context = context
        self._bridge_executor = get_bridge_executor()
        # 工具输出格式偏好
        self._response_format = response_format if response_format in ("text", "content_and_artifact") else "text"

    @staticmethod
    def _serialize_unknown(obj):
        """序列化未知类型对象。"""
        if obj is None:
            return None
        if hasattr(obj, "model_dump"):
            try:
                return obj.model_dump()
            except Exception:
                pass
        if hasattr(obj, "dict"):
            try:
                return obj.dict()
            except Exception:
                pass
        if is_dataclass(obj):
            try:
                return asdict(obj)
            except Exception:
                pass
        if hasattr(obj, "__dict__"):
            try:
                return {k: v for k, v in obj.__dict__.items() if not k.startswith("_")}
            except Exception:
                pass
        return str(obj)

    def _normalize_structured_value(self, value):
        """确保 structured/data 字段始终是 LangChain 能消费的基础类型。"""
        if value is None:
            return None
        if isinstance(value, (str, int, float, bool)):
            return value
        if isinstance(value, (dict, list)):
            return value
        try:
            return json.loads(json.dumps(value, default=self._serialize_unknown, ensure_ascii=False))
        except Exception:
            return str(value)

    def _create_tool_function(self, tool_name: str, args_schema: Type[BaseModel]):
        """
        创建健壮的同步执行函数，智能处理各种参数传递方式。
        """
        adapter_self = self  # 闭包捕获

        def _tool_executor(*args, **kwargs):
            try:
                # 使用公共函数处理参数
                tool_input = process_tool_args(args_schema, args, kwargs)

                # 调用 mcpstore 核心方法
                result = adapter_self._context.call_tool(tool_name, tool_input)
                view = call_tool_response_helper(result)

                if view.is_error:
                    raise ToolException(view.error_message or view.text or "Tool execution failed")

                if adapter_self._response_format == "content_and_artifact":
                    response = {"text": view.text, "artifacts": view.artifacts}
                    structured = adapter_self._normalize_structured_value(view.structured)
                    data = adapter_self._normalize_structured_value(view.data)
                    if structured is not None:
                        response["structured"] = structured
                    if data is not None:
                        response["data"] = data
                    return response

                return view.text

            except ToolException:
                raise
            except Exception as e:
                error_msg = f"Tool '{tool_name}' execution failed: {str(e)}"
                if args or kwargs:
                    error_msg += f"\nParameter info: args={args}, kwargs={kwargs}"
                if tool_input:
                    error_msg += f"\nProcessed parameters: {tool_input}"
                return error_msg

        return _tool_executor

    async def _create_tool_coroutine(self, tool_name: str, args_schema: Type[BaseModel]):
        """
        创建健壮的异步执行函数，智能处理各种参数传递方式。
        """
        adapter_self = self  # 闭包捕获

        async def _tool_executor(*args, **kwargs):
            try:
                # 使用公共函数处理参数
                tool_input = process_tool_args(args_schema, args, kwargs)

                # 调用 mcpstore 核心方法（异步版本）
                result = await adapter_self._context.call_tool_async(tool_name, tool_input)
                view = call_tool_response_helper(result)

                if view.is_error:
                    raise ToolException(view.error_message or view.text or "Tool execution failed")

                if adapter_self._response_format == "content_and_artifact":
                    response = {"text": view.text, "artifacts": view.artifacts}
                    structured = adapter_self._normalize_structured_value(view.structured)
                    data = adapter_self._normalize_structured_value(view.data)
                    if structured is not None:
                        response["structured"] = structured
                    if data is not None:
                        response["data"] = data
                    return response

                return view.text

            except ToolException:
                raise
            except Exception as e:
                error_msg = f"Tool '{tool_name}' execution failed: {str(e)}"
                if args or kwargs:
                    error_msg += f"\nParameter info: args={args}, kwargs={kwargs}"
                if tool_input:
                    error_msg += f"\nProcessed parameters: {tool_input}"
                return error_msg

        return _tool_executor

    def list_tools(self) -> List[Tool]:
        """获取所有可用的 mcpstore 工具并转换为 LangChain Tool 列表（同步版本）。"""
        return self._bridge_executor.run_sync(
            self.list_tools_async(),
            op_name="LangChainAdapter.list_tools",
        )

    async def list_tools_async(self) -> List[Tool]:
        """
        获取所有可用的 mcpstore 工具并转换为 LangChain Tool 列表（异步版本）。

        Raises:
            RuntimeError: 如果没有可用工具（所有服务连接失败）
        """
        mcp_tools_info = await self._context.list_tools_async()

        # 检查工具是否为空，提供友好的错误信息
        if not mcp_tools_info:
            logger.warning("[LIST_TOOLS] empty=True")
            services = await self._context.list_services_async()
            if not services:
                raise RuntimeError(
                    "No available tools: No MCP services have been added. "
                    "Please add services using add_service() first."
                )
            else:
                failed_services = [s.name for s in services if s.status.value != 'healthy']
                if failed_services:
                    raise RuntimeError(
                        f"No available tools: The following services failed to connect: {', '.join(failed_services)}. "
                        f"Please check service configuration and dependencies, or use wait_service() to wait for services to be ready. "
                        f"\nTip: You can use list_services() to view detailed service status."
                    )
                else:
                    raise RuntimeError(
                        "No available tools: Services are connected but provide no tools. "
                        "Please check if services are working properly."
                    )

        langchain_tools = []
        for tool_info in mcp_tools_info:
            # 使用公共函数
            enhanced_description = enhance_description(tool_info)
            args_schema = create_args_schema(tool_info)

            # 创建同步和异步函数
            sync_func = self._create_tool_function(tool_info.name, args_schema)
            async_coroutine = await self._create_tool_coroutine(tool_info.name, args_schema)

            # 根据原始 schema 确定参数数量
            schema_properties = tool_info.inputSchema.get("properties", {})
            original_param_count = len(schema_properties)

            # 读取工具覆盖配置（如 return_direct）
            try:
                return_direct_flag = self._context._get_tool_override(
                    tool_info.service_name, tool_info.name, "return_direct", False
                )
            except Exception:
                return_direct_flag = False

            # 创建 LangChain StructuredTool
            lc_tool = StructuredTool(
                name=tool_info.name,
                description=enhanced_description,
                func=sync_func,
                coroutine=async_coroutine,
                args_schema=args_schema,
            )

            # 设置 return_direct
            try:
                setattr(lc_tool, 'return_direct', bool(return_direct_flag))
            except Exception:
                pass

            langchain_tools.append(lc_tool)

        return langchain_tools


class SessionAwareLangChainAdapter(LangChainAdapter):
    """
    会话感知的 LangChain 适配器。

    此增强适配器创建绑定到特定会话的 LangChain 工具，
    确保在 LangChain agent 工作流中的多次工具调用之间保持状态。

    主要特性：
    - 工具自动使用会话绑定执行
    - 跨工具调用保持状态（如浏览器保持打开）
    - 与现有 LangChain 工作流无缝集成
    - 向后兼容标准 LangChainAdapter
    """

    def __init__(self, context: 'MCPStoreContext', session: 'Session', response_format: str = "text"):
        """
        初始化会话感知适配器。

        Args:
            context: MCPStoreContext 实例（用于工具发现）
            session: 工具将绑定到的会话对象
            response_format: 同 LangChainAdapter（"text" 或 "content_and_artifact"）
        """
        super().__init__(context, response_format=response_format)
        self._session = session
        logger.debug(f"Initialized session-aware adapter for session '{session.session_id}'")

    def _create_tool_function(self, tool_name: str, args_schema: Type[BaseModel]):
        """
        创建会话绑定的工具函数。

        覆盖父类方法，通过会话路由工具执行，确保跨工具调用的状态持久化。
        """
        adapter_self = self  # 闭包捕获

        def _session_tool_executor(*args, **kwargs):
            try:
                # 使用公共函数处理参数
                tool_input = process_tool_args(args_schema, args, kwargs)

                # [关键] 使用会话绑定执行而不是 context.call_tool
                logger.debug(
                    f"[SESSION_LANGCHAIN] Executing tool '{tool_name}' "
                    f"via session '{adapter_self._session.session_id}'"
                )
                result = adapter_self._session.use_tool(tool_name, tool_input)
                view = call_tool_response_helper(result)

                if view.is_error:
                    raise ToolException(view.error_message or view.text or "Tool execution failed")

                if adapter_self._response_format == "content_and_artifact":
                    response = {"text": view.text, "artifacts": view.artifacts}
                    structured = adapter_self._normalize_structured_value(view.structured)
                    data = adapter_self._normalize_structured_value(view.data)
                    if structured is not None:
                        response["structured"] = structured
                    if data is not None:
                        response["data"] = data
                    return response

                return view.text

            except ToolException:
                raise
            except Exception as e:
                error_msg = f"Tool execution failed: {str(e)}"
                logger.error(f"[SESSION_LANGCHAIN] {error_msg}")
                return error_msg

        return _session_tool_executor

    def _create_async_tool_function(self, tool_name: str, args_schema: Type[BaseModel]):
        """创建会话绑定的异步工具函数。"""
        adapter_self = self  # 闭包捕获

        async def _session_async_tool_executor(*args, **kwargs):
            try:
                # 使用公共函数处理参数
                tool_input = process_tool_args(args_schema, args, kwargs)

                # [关键] 使用会话绑定异步执行
                logger.debug(
                    f"[SESSION_LANGCHAIN] Executing tool '{tool_name}' "
                    f"via session '{adapter_self._session.session_id}' (async)"
                )
                result = await adapter_self._session.use_tool_async(tool_name, tool_input)
                view = call_tool_response_helper(result)

                if view.is_error:
                    raise ToolException(view.error_message or view.text or "Tool execution failed")

                if adapter_self._response_format == "content_and_artifact":
                    response = {"text": view.text, "artifacts": view.artifacts}
                    structured = adapter_self._normalize_structured_value(view.structured)
                    data = adapter_self._normalize_structured_value(view.data)
                    if structured is not None:
                        response["structured"] = structured
                    if data is not None:
                        response["data"] = data
                    return response

                return view.text

            except ToolException:
                raise
            except Exception as e:
                error_msg = f"Async tool execution failed: {str(e)}"
                logger.error(f"[SESSION_LANGCHAIN] {error_msg}")
                return error_msg

        return _session_async_tool_executor

    async def list_tools_async(self) -> List[Tool]:
        """
        创建会话绑定的 LangChain 工具（异步版本）。

        Returns:
            绑定到会话的 LangChain Tool 对象列表
        """
        logger.debug(f"Creating session-bound tools for session '{self._session.session_id}'")

        # 使用父类的工具发现逻辑
        mcpstore_tools = await self._context.list_tools_async()
        langchain_tools = []

        for tool_info in mcpstore_tools:
            # 使用公共函数
            args_schema = create_args_schema(tool_info)
            enhanced_description = enhance_description(tool_info)

            # 创建会话绑定函数
            sync_func = self._create_tool_function(tool_info.name, args_schema)
            async_coroutine = self._create_async_tool_function(tool_info.name, args_schema)

            # 创建带会话绑定的 LangChain 工具
            langchain_tools.append(
                StructuredTool(
                    name=tool_info.name,
                    description=enhanced_description + f" [Session: {self._session.session_id}]",
                    func=sync_func,
                    coroutine=async_coroutine,
                    args_schema=args_schema,
                )
            )

        logger.debug(f"Created {len(langchain_tools)} session-bound tools")
        return langchain_tools

    def list_tools(self) -> List[Tool]:
        """
        创建会话绑定的 LangChain 工具（同步版本）。

        Returns:
            绑定到会话的 LangChain Tool 对象列表
        """
        return self._context._bridge.run(
            self.list_tools_async(), op_name="LangChainAdapter.list_tools_for_session"
        )
