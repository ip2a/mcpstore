"""
MCPStore Tool Proxy Module
工具代理对象，提供具体工具的操作方法
"""

import logging
from datetime import datetime
from typing import Dict, List, Optional, Any, TYPE_CHECKING

from .types import ContextType

if TYPE_CHECKING:
    from ..models.tool import ToolInfo

logger = logging.getLogger(__name__)


class ToolCallResult:
    """
    工具调用结果封装
    基于 FastMCP CallToolResult 提供友好接口
    """
    
    def __init__(self, fastmcp_result, tool_name: str, arguments: Dict[str, Any]):
        """
        初始化工具调用结果
        
        Args:
            fastmcp_result: FastMCP 的 CallToolResult 对象
            tool_name: 工具名称
            arguments: 调用参数
        """
        self._result = fastmcp_result
        self._tool_name = tool_name
        self._arguments = arguments
        self._called_at = datetime.now()
        
        logger.debug(f"[TOOL_CALL_RESULT] Created for tool '{tool_name}', error={self.is_error}")
    
    @property
    def data(self):
        """
        FastMCP 的完全水合对象（核心特色）
        
        Returns:
            Any: 完全重构的 Python 对象，包括复杂类型如 datetime、UUID 等
        """
        return self._result.data if hasattr(self._result, 'data') else None
    
    @property
    def content(self):
        """
        标准 MCP 内容块
        
        Returns:
            List: MCP 内容块列表 (TextContent, ImageContent 等)
        """
        return self._result.content if hasattr(self._result, 'content') else []
        
    @property
    def structured_content(self) -> Optional[Dict[str, Any]]:
        """
        标准 MCP 结构化 JSON 数据
        
        Returns:
            Dict: 服务器发送的原始结构化数据
        """
        return getattr(self._result, 'structured_content', None)
        
    @property
    def is_error(self) -> bool:
        """
        是否出错
        
        Returns:
            bool: True 表示工具执行失败
        """
        return getattr(self._result, 'is_error', False)
        
    @property
    def text_output(self) -> str:
        """
        便捷的文本输出
        
        Returns:
            str: 工具的文本结果
        """
        if self.content and len(self.content) > 0:
            first_content = self.content[0]
            if hasattr(first_content, 'text'):
                return first_content.text
        
        # 如果没有文本内容，尝试从 data 获取
        if self.data is not None:
            return str(self.data)
            
        return ""
    
    @property
    def tool_name(self) -> str:
        """获取工具名称"""
        return self._tool_name
    
    @property
    def arguments(self) -> Dict[str, Any]:
        """获取调用参数"""
        return self._arguments
    
    @property
    def called_at(self) -> datetime:
        """获取调用时间"""
        return self._called_at
    
    def to_dict(self) -> Dict[str, Any]:
        """
        转换为字典格式
        
        Returns:
            Dict: 包含所有结果信息的字典
        """
        return {
            "tool_name": self.tool_name,
            "arguments": self.arguments,
            "called_at": self.called_at.isoformat(),
            "is_error": self.is_error,
            "data": self.data,
            "text_output": self.text_output,
            "has_structured_content": self.structured_content is not None
        }
    
    def __str__(self) -> str:
        status = "ERROR" if self.is_error else "SUCCESS"
        return f"ToolCallResult(tool='{self.tool_name}', status={status}, output='{self.text_output[:50]}...')"
    
    def __repr__(self) -> str:
        return self.__str__()

    def find_cache(self) -> "CacheProxy":
        from .cache_proxy import CacheProxy
        return CacheProxy(self._context, scope="tool", scope_value=self._tool_name)


class ToolProxy:
    """
    工具代理对象
    提供具体工具的所有操作方法，进一步缩小作用域
    """

    def __init__(self, context: 'MCPStoreContext', tool_name: str, 
                 scope: str = 'context', service_name: str = None):
        """
        初始化工具代理
        
        Args:
            context: 父级上下文对象
            tool_name: 工具名称
            scope: 作用域类型 ('context' | 'service')
            service_name: 服务名称 (当 scope='service' 时)
        """
        self._context = context
        self._tool_name = tool_name
        self._scope = scope
        self._service_name = service_name
        self._context_type = context.context_type
        self._agent_id = context.agent_id
        self._tool_info = None  # 延迟加载
        self._tool_info_obj: Optional['ToolInfo'] = None  # 精准的 ToolInfo 对象缓存

        logger.debug(f"[TOOL_PROXY] Created proxy for tool '{tool_name}' "
                    f"in {self._context_type.value} context, scope={scope}, service={service_name}")

    @property
    def tool_name(self) -> str:
        """获取工具名称"""
        return self._tool_name

    @property
    def context_type(self) -> ContextType:
        """获取上下文类型"""
        return self._context_type
    
    @property
    def scope(self) -> str:
        """获取作用域类型"""
        return self._scope
    
    @property
    def service_name(self) -> Optional[str]:
        """获取关联的服务名称"""
        return self._service_name

    # === 工具信息查询方法（两个单词）===

    def tool_info(self) -> Dict[str, Any]:
        """
        获取工具详细信息（包括 FastMCP 的 meta 和 tags）
        
        Returns:
            Dict: 工具的完整信息，包括 FastMCP 特有的 meta 数据
        """
        if not self._tool_info:
            self._load_tool_info()
        
        return self._tool_info or {}

    def tool_schema(self) -> Optional[Dict[str, Any]]:
        """
        获取工具参数模式
        
        Returns:
            Dict: 工具的输入参数 schema
        """
        info = self.tool_info()
        return info.get('inputSchema')

    def tool_tags(self) -> List[str]:
        """
        获取工具标签（基于 FastMCP meta._fastmcp.tags）
        
        Returns:
            List[str]: 工具标签列表
        """
        info = self.tool_info()
        return info.get('tags', [])

    def tool_meta(self) -> Dict[str, Any]:
        """
        获取工具元数据
        
        Returns:
            Dict: 完整的 meta 数据
        """
        info = self.tool_info()
        return info.get('meta', {})

    # === FastMCP 视图 ===

    def mcp_type2tool(self):
        """
        将当前工具转换为 FastMCP 官方的 Tool 对象
        """
        from mcp import types as mcp_types

        tool = self._get_tool_info_object()
        if not tool:
            # TODO: 工具命中机制待评估，避免模糊匹配导致返回错误的 FastMCP Tool。
            raise ValueError(f"Tool '{self._tool_name}' not found; cannot build mcp.types.Tool")

        meta = {
            "service_name": tool.service_name,
            "service_global_name": tool.service_global_name,
            "client_id": tool.client_id,
        }

        return mcp_types.Tool(
            name=tool.tool_original_name or tool.name,
            title=getattr(tool, "title", None) or tool.name,
            description=tool.description,
            inputSchema=tool.inputSchema or {},
            outputSchema=getattr(tool, "outputSchema", None),
            icons=getattr(tool, "icons", None),
            annotations=getattr(tool, "annotations", None),
            _meta=meta,
        )

    # === 配置覆盖（如 LangChain return_direct） ===

    def set_redirect(self, enabled: bool = True) -> 'ToolProxy':
        """
        标记该工具为 "redirect" 行为（LangChain 中对应 return_direct）。

        当后续通过 context.for_langchain().list_tools() 转换为 LangChain 工具时，
        将读取该标记并设置到生成的 Tool/StructuredTool 上。
        """
        try:
            # 1) 先尝试加载精确的工具信息
            if not self._tool_info:
                self._load_tool_info()

            resolved_service = None
            resolved_tool_name = None

            if self._tool_info:
                # 已经有精确匹配的信息
                resolved_service = self._tool_info.get('service_name')
                resolved_tool_name = self._tool_info.get('name', self._tool_name)
            else:
                # 2) 进行后缀匹配解析：支持传入简名（如 get_current_weather）
                tools = self._context._run_async_via_bridge(
                    self._context.list_tools_async(),
                    op_name="tool_proxy.set_redirect.list_tools"
                )
                candidate = None

                for t in tools:
                    # 限定服务匹配（如果指定了 service 范围）
                    if self._service_name and t.service_name != self._service_name:
                        continue

                    if t.name == self._tool_name:
                        candidate = t
                        break
                    # 支持下划线或双下划线分隔的后缀匹配
                    if t.name.endswith(f"_{self._tool_name}") or t.name.endswith(f"__{self._tool_name}"):
                        candidate = t
                        # 不立即 break，以便优先找到完全相同服务的匹配项（上面已按服务过滤）

                if candidate:
                    resolved_service = candidate.service_name
                    resolved_tool_name = candidate.name
                else:
                    # 保底：直接使用现有信息（可能覆盖不到正确键）
                    resolved_service = self._service_name or ""
                    resolved_tool_name = self._tool_name

            # 3) 设置覆盖键（service:resolved_tool_name）
            self._context._set_tool_override(resolved_service or "", resolved_tool_name, "return_direct", bool(enabled))
            logger.debug(
                f"[TOOL_PROXY] set_redirect(return_direct)={enabled} input='{self._tool_name}', "
                f"resolved='{resolved_tool_name}', service='{resolved_service}'"
            )
        except Exception as e:
            logger.warning(f"[TOOL_PROXY] set_redirect failed: {e}")
        return self

    # === 工具执行方法（两个单词）===

    def call_tool(self, arguments: Dict[str, Any] = None, return_extracted: bool = False, **kwargs) -> Any:
        """
        调用工具（同步版本）
        利用 FastMCP 的 call_tool() 和 CallToolResult
        
        Args:
            arguments: 工具参数字典
            **kwargs: 额外的调用选项 (timeout, progress_handler 等)
        
        Returns:
            Any: FastMCP CallToolResult（或当 return_extracted=True 时返回已提取的数据）
        """
        return self._context._run_async_via_bridge(
            self.call_tool_async(arguments, return_extracted=return_extracted, **kwargs),
            op_name="tool_proxy.call_tool"
        )

    async def call_tool_async(self, arguments: Dict[str, Any] = None, return_extracted: bool = False, **kwargs) -> Any:
        """
        调用工具（异步版本）
        
        Args:
            arguments: 工具参数字典
            **kwargs: 额外的调用选项 (timeout, progress_handler 等)
        
        Returns:
            Any: FastMCP CallToolResult（或当 return_extracted=True 时返回已提取的数据）
        """
        arguments = arguments or {}
        logger.info(f"[TOOL_PROXY] Calling tool '{self._tool_name}' with args: {arguments}")
        return await self._context.call_tool_async(self._tool_name, arguments, return_extracted=return_extracted, **kwargs)

    def test_call(self, arguments: Dict[str, Any] = None, return_extracted: bool = False) -> Any:
        """
        测试调用工具（包含验证逻辑）
        
        Args:
            arguments: 测试参数
            
        Returns:
            Any: FastMCP CallToolResult（或当 return_extracted=True 时返回已提取的数据）
        """
        # 首先验证工具是否存在
        info = self.tool_info()
        if not info:
            raise ValueError(f"Tool '{self._tool_name}' not found")
        
        # 执行实际调用
        return self.call_tool(arguments, return_extracted=return_extracted)

    # === 工具统计方法（两个单词）===

    def usage_stats(self) -> Dict[str, Any]:
        """
        获取该工具的使用统计
        
        Returns:
            Dict: 工具使用统计信息
        """
        try:
            # 通过监控系统获取工具统计
            if hasattr(self._context, '_monitoring') and self._context._monitoring:
                # 获取工具使用记录
                records = self._context._monitoring.get_tool_records(limit=100)
                
                # 过滤当前工具的记录（新结构键为 executions）
                executions = records.get('executions', [])
                tool_records = [
                    record for record in executions
                    if record.get('tool_name') == self._tool_name
                ]
                warning_msg = records.get('warning')
                
                return {
                    "tool_name": self._tool_name,
                    "total_calls": len(tool_records),
                    "recent_calls": len([r for r in tool_records[-10:]]),  # 最近10次
                    "success_rate": self._calculate_success_rate(tool_records),
                    "average_duration": self._calculate_average_duration(tool_records),
                    **({"degraded": warning_msg} if warning_msg else {})
                }
            else:
                return {
                    "tool_name": self._tool_name,
                    "total_calls": 0,
                    "recent_calls": 0,
                    "success_rate": 0.0,
                    "average_duration": 0.0,
                    "note": "Monitoring not available"
                }
        except Exception as e:
            logger.error(f"[TOOL_PROXY] Failed to get usage stats: {e}")
            return {
                "tool_name": self._tool_name,
                "error": str(e)
            }

    def call_history(self, limit: int = 10) -> List[Dict[str, Any]]:
        """
        获取调用历史
        
        Args:
            limit: 返回记录数量限制
            
        Returns:
            List[Dict]: 调用历史记录
        """
        try:
            if hasattr(self._context, '_monitoring') and self._context._monitoring:
                records = self._context._monitoring.get_tool_records(limit=limit * 2)  # 获取更多记录用于过滤
                
                # 过滤当前工具的记录（新结构键为 executions）
                executions = records.get('executions', [])
                tool_records = [
                    record for record in executions
                    if record.get('tool_name') == self._tool_name
                ]
                if records.get('warning'):
                    tool_records.append({"degraded": records.get('warning')})
                
                # 返回最近的记录
                return tool_records[:limit]
            else:
                return []
        except Exception as e:
            logger.error(f"[TOOL_PROXY] Failed to get call history: {e}")
            return []

    # === 内部辅助方法 ===

    def _load_tool_info(self):
        """延迟加载工具信息"""
        try:
            # 获取所有工具信息
            tools = self._context._run_async_via_bridge(
                self._context.list_tools_async(),
                op_name="tool_proxy.load_tool_info.list_tools"
            )
            
            for tool in tools:
                if tool.name == self._tool_name:
                    # 如果是服务范围，验证服务匹配
                    if self._scope == 'service' and self._service_name:
                        if tool.service_name != self._service_name:
                            continue
                    
                    # 构建工具信息
                    info: Dict[str, Any] = {
                        'name': tool.name,
                        'description': tool.description,
                        'inputSchema': tool.inputSchema,
                        'service_name': tool.service_name,
                        'client_id': tool.client_id,
                        'tags': [],
                        'meta': {},
                        'scope': self._scope
                    }
                    # 1) 从 pykv 实体层补充 original_name/display_name（不使用快照）
                    try:
                        # 直接从 pykv 实体层获取工具实体
                        tool_entity_manager = self._context._store.registry._cache_tool_manager
                        tool_entity = self._context._run_async_via_bridge(
                            tool_entity_manager.get_tool(tool.name),
                            op_name="tool_proxy.load_tool_info.get_tool_entity"
                        )
                        if tool_entity:
                            entity_dict = tool_entity.to_dict() if hasattr(tool_entity, 'to_dict') else tool_entity
                            if 'tool_original_name' in entity_dict:
                                info['original_name'] = entity_dict.get('tool_original_name')
                            # display_name 默认使用 original_name
                            if 'original_name' in info:
                                info['display_name'] = info.get('original_name')
                    except Exception as e:
                        logger.debug(f"[TOOL_PROXY] pykv enrichment failed: {e}")

                    # 2) 从转换管理器补充 tags（如有）
                    try:
                        tm = getattr(self._context, '_transformation_manager', None)
                        if tm and hasattr(tm, 'transformer') and hasattr(tm.transformer, 'get_transformation_config'):
                            # 优先使用显示名（tool.name），其次 original_name
                            cfg = tm.transformer.get_transformation_config(tool.name)
                            if not cfg and isinstance(info.get('original_name'), str):
                                cfg = tm.transformer.get_transformation_config(info.get('original_name'))
                            if cfg and hasattr(cfg, 'tags') and isinstance(cfg.tags, list):
                                # 合并且去重
                                existing = set(info.get('tags') or [])
                                for t in cfg.tags:
                                    if isinstance(t, str):
                                        existing.add(t)
                                info['tags'] = list(existing)
                    except Exception as e:
                        logger.debug(f"[TOOL_PROXY] transformation tags enrichment failed: {e}")

                    # 3) 将 tags 写入 meta._fastmcp.tags，以兼容“FastMCP meta._fastmcp.tags”读取预期
                    try:
                        tags = info.get('tags') or []
                        meta = info.get('meta') or {}
                        if not isinstance(meta, dict):
                            meta = {}
                        fm = meta.get('_fastmcp') or {}
                        if not isinstance(fm, dict):
                            fm = {}
                        if isinstance(tags, list):
                            fm['tags'] = tags
                        meta['_fastmcp'] = fm
                        info['meta'] = meta
                    except Exception as e:
                        logger.debug(f"[TOOL_PROXY] meta/_fastmcp tags merge failed: {e}")

                    self._tool_info = info
                    
                    break
            
            if not self._tool_info:
                logger.debug(f"[TOOL_PROXY] Tool '{self._tool_name}' not found in scope '{self._scope}'")
                
        except Exception as e:
            logger.error(f"[TOOL_PROXY] Failed to load tool info: {e}")

    def _get_tool_info_object(self) -> Optional['ToolInfo']:
        """
        获取完整的 ToolInfo 对象，用于构造 FastMCP Tool
        """
        if self._tool_info_obj is not None:
            return self._tool_info_obj

        try:
            tools: List['ToolInfo'] = self._context._run_async_via_bridge(
                self._context.list_tools_async(),
                op_name="tool_proxy.get_tool_info_object.list_tools"
            )
            for tool in tools:
                if self._service_name and tool.service_name != self._service_name:
                    continue

                if (
                    tool.name == self._tool_name
                    or (tool.tool_original_name and tool.tool_original_name == self._tool_name)
                    or tool.name.endswith(f"_{self._tool_name}")
                    or tool.name.endswith(f"__{self._tool_name}")
                ):
                    self._tool_info_obj = tool
                    break
        except Exception as exc:
            logger.error(f"[TOOL_PROXY] Failed to resolve ToolInfo object: {exc}")

        return self._tool_info_obj or None

    def _calculate_success_rate(self, records: List[Dict[str, Any]]) -> float:
        """计算成功率"""
        if not records:
            return 0.0
        
        success_count = sum(1 for record in records if not record.get('is_error', False))
        return (success_count / len(records)) * 100.0

    def _calculate_average_duration(self, records: List[Dict[str, Any]]) -> float:
        """计算平均执行时间"""
        if not records:
            return 0.0
        
        durations = [record.get('duration', 0.0) for record in records if 'duration' in record]
        if not durations:
            return 0.0
        
        return sum(durations) / len(durations)

    # === 便捷属性方法 ===

    @property
    def name(self) -> str:
        """获取工具名称（便捷属性）"""
        return self._tool_name

    @property
    def description(self) -> str:
        """获取工具描述"""
        info = self.tool_info()
        return info.get('description', '')

    @property
    def has_schema(self) -> bool:
        """是否有参数模式"""
        return self.tool_schema() is not None

    @property
    def is_available(self) -> bool:
        """工具是否可用"""
        return bool(self.tool_info())

    def __str__(self) -> str:
        scope_info = f", service='{self._service_name}'" if self._scope == 'service' else ""
        return f"ToolProxy(tool='{self._tool_name}', context='{self._context_type.value}', scope='{self._scope}'{scope_info})"

    def __repr__(self) -> str:
        return self.__str__()
