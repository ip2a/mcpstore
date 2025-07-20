#!/usr/bin/env python3
"""
统一工具名称解析器 - 基于 FastMCP 官网标准
提供用户友好的工具名称输入，内部转换为 FastMCP 标准格式
"""

import re
import logging
from typing import Tuple, Optional, List, Dict, Any
from dataclasses import dataclass

logger = logging.getLogger(__name__)

@dataclass
class ToolResolution:
    """工具解析结果"""
    service_name: str           # 服务名称
    original_tool_name: str     # FastMCP 标准的原始工具名
    user_input: str            # 用户输入的工具名
    resolution_method: str     # 解析方法 (exact_match, prefix_match, fuzzy_match)

class ToolNameResolver:
    """
    统一工具名称解析器

    设计原则：
    1. 用户友好：支持多种输入格式
    2. FastMCP 标准：内部严格按照官网标准处理
    3. 智能解析：自动识别服务和工具
    4. 服务名校验：使用单下划线 + 精确服务名匹配
    """

    def __init__(self, available_services: List[str] = None):
        """
        初始化解析器

        Args:
            available_services: 可用服务列表，用于智能匹配
        """
        self.available_services = available_services or []
        self._service_tools_cache: Dict[str, List[str]] = {}

        # 预处理服务名映射（原始名 -> 标准化名）
        self._service_name_mapping = {}
        for service in self.available_services:
            normalized = self._normalize_service_name(service)
            self._service_name_mapping[normalized] = service
            # 同时支持原始名称
            self._service_name_mapping[service] = service
    
    def resolve_tool_name(self, user_input: str, available_tools: List[Dict[str, Any]] = None) -> ToolResolution:
        """
        解析用户输入的工具名称

        Args:
            user_input: 用户输入的工具名称
            available_tools: 可用工具列表 [{"name": "display_name", "original_name": "tool", "service_name": "service"}]

        Returns:
            ToolResolution: 解析结果

        Raises:
            ValueError: 无法解析工具名称
        """
        if not user_input or not isinstance(user_input, str):
            raise ValueError("Tool name cannot be empty")

        user_input = user_input.strip()
        available_tools = available_tools or []

        # 构建工具映射（支持显示名称和原始名称）
        display_to_original = {}  # 显示名称 -> (原始名称, 服务名)
        original_to_service = {}  # 原始名称 -> 服务名
        service_tools = {}        # 服务名 -> [原始工具名列表]

        for tool in available_tools:
            display_name = tool.get("name", "")  # 显示名称
            original_name = tool.get("original_name") or tool.get("name", "")  # 原始名称
            service_name = tool.get("service_name", "")

            display_to_original[display_name] = (original_name, service_name)
            original_to_service[original_name] = service_name

            if service_name not in service_tools:
                service_tools[service_name] = []
            if original_name not in service_tools[service_name]:
                service_tools[service_name].append(original_name)

        logger.debug(f"Resolving tool: {user_input}")
        logger.debug(f"Available services: {list(service_tools.keys())}")

        # 1. 精确匹配：显示名称
        if user_input in display_to_original:
            original_name, service_name = display_to_original[user_input]
            return ToolResolution(
                service_name=service_name,
                original_tool_name=original_name,
                user_input=user_input,
                resolution_method="exact_display_match"
            )

        # 2. 精确匹配：原始名称
        if user_input in original_to_service:
            return ToolResolution(
                service_name=original_to_service[user_input],
                original_tool_name=user_input,
                user_input=user_input,
                resolution_method="exact_original_match"
            )

        # 3. 单下划线格式解析：service_tool（精确服务名匹配）
        if "_" in user_input and "__" not in user_input:
            # 尝试所有可能的分割点
            for i in range(1, len(user_input)):
                if user_input[i] == "_":
                    potential_service = user_input[:i]
                    potential_tool = user_input[i+1:]

                    # 检查是否有匹配的服务（支持原始名称和标准化名称）
                    matched_service = None
                    if potential_service in service_tools:
                        matched_service = potential_service
                    elif potential_service in self._service_name_mapping:
                        matched_service = self._service_name_mapping[potential_service]

                    if matched_service and potential_tool in service_tools[matched_service]:
                        logger.debug(f"Single underscore match: {potential_service} -> {matched_service}, tool: {potential_tool}")
                        return ToolResolution(
                            service_name=matched_service,
                            original_tool_name=potential_tool,
                            user_input=user_input,
                            resolution_method="single_underscore_match"
                        )

        # 4. 检查是否使用了废弃的双下划线格式
        if "__" in user_input:
            parts = user_input.split("__", 1)
            if len(parts) == 2:
                potential_service, potential_tool = parts
                single_underscore_format = f"{potential_service}_{potential_tool}"
                raise ValueError(
                    f"Double underscore format '__' is no longer supported. "
                    f"Please use single underscore format: '{single_underscore_format}'"
                )

        # 5. 模糊匹配：在所有工具中查找相似名称
        fuzzy_matches = []
        for display_name, (original_name, service_name) in display_to_original.items():
            if self._is_fuzzy_match(user_input, display_name) or self._is_fuzzy_match(user_input, original_name):
                fuzzy_matches.append((original_name, service_name, display_name))

        if len(fuzzy_matches) == 1:
            original_name, service_name, display_name = fuzzy_matches[0]
            return ToolResolution(
                service_name=service_name,
                original_tool_name=original_name,
                user_input=user_input,
                resolution_method="fuzzy_match"
            )
        elif len(fuzzy_matches) > 1:
            # 多个匹配，提供建议
            suggestions = [display_name for _, _, display_name in fuzzy_matches[:3]]
            raise ValueError(f"Ambiguous tool name '{user_input}'. Did you mean: {', '.join(suggestions)}?")

        # 6. 无法解析，提供建议
        if available_tools:
            all_display_names = list(display_to_original.keys())
            suggestions = self._get_suggestions(user_input, all_display_names)
            if suggestions:
                raise ValueError(f"Tool '{user_input}' not found. Did you mean: {', '.join(suggestions[:3])}?")

        raise ValueError(f"Tool '{user_input}' not found")
    
    def create_user_friendly_name(self, service_name: str, tool_name: str) -> str:
        """
        创建用户友好的工具名称（用于显示）

        使用单下划线格式，保持服务名的原始形式

        Args:
            service_name: 服务名称（保持原始格式）
            tool_name: 原始工具名称

        Returns:
            用户友好的工具名称
        """
        # 使用单下划线，保持服务名原始格式
        return f"{service_name}_{tool_name}"
    
    def _normalize_service_name(self, service_name: str) -> str:
        """标准化服务名称"""
        # 移除特殊字符，转换为下划线
        normalized = re.sub(r'[^a-zA-Z0-9_]', '_', service_name)
        # 移除连续下划线
        normalized = re.sub(r'_+', '_', normalized)
        # 移除首尾下划线
        normalized = normalized.strip('_')
        return normalized or "unnamed"
    
    def _is_fuzzy_match(self, user_input: str, tool_name: str) -> bool:
        """检查是否为模糊匹配"""
        user_lower = user_input.lower()
        tool_lower = tool_name.lower()
        
        # 完全包含
        if user_lower in tool_lower or tool_lower in user_lower:
            return True
        
        # 去除下划线后匹配
        user_clean = user_lower.replace('_', '').replace('-', '')
        tool_clean = tool_lower.replace('_', '').replace('-', '')
        
        if user_clean in tool_clean or tool_clean in user_clean:
            return True
        
        return False
    
    def _get_suggestions(self, user_input: str, available_names: List[str]) -> List[str]:
        """获取建议的工具名称"""
        suggestions = []
        user_lower = user_input.lower()
        
        for name in available_names:
            name_lower = name.lower()
            # 前缀匹配
            if name_lower.startswith(user_lower) or user_lower.startswith(name_lower):
                suggestions.append(name)
            # 包含匹配
            elif user_lower in name_lower or name_lower in user_lower:
                suggestions.append(name)
        
        return sorted(suggestions, key=lambda x: len(x))[:5]

class FastMCPToolExecutor:
    """
    FastMCP 标准工具执行器
    严格按照官网标准执行工具调用
    """
    
    def __init__(self, default_timeout: float = 30.0):
        """
        初始化执行器
        
        Args:
            default_timeout: 默认超时时间（秒）
        """
        self.default_timeout = default_timeout
    
    async def execute_tool(
        self,
        client,
        tool_name: str,
        arguments: Dict[str, Any] = None,
        timeout: Optional[float] = None,
        progress_handler = None,
        raise_on_error: bool = True
    ) -> 'CallToolResult':
        """
        执行工具（严格按照 FastMCP 官网标准）
        
        Args:
            client: FastMCP 客户端实例
            tool_name: 工具名称（FastMCP 原始名称）
            arguments: 工具参数
            timeout: 超时时间（秒）
            progress_handler: 进度处理器
            raise_on_error: 是否在错误时抛出异常
            
        Returns:
            CallToolResult: FastMCP 标准结果对象
        """
        arguments = arguments or {}
        timeout = timeout or self.default_timeout
        
        try:
            # 根据实际的 FastMCP 2.7.1 版本调用
            call_kwargs = {
                "name": tool_name,
                "arguments": arguments
            }

            # 添加支持的参数
            if timeout is not None:
                call_kwargs["timeout"] = timeout
            if progress_handler is not None:
                call_kwargs["progress_handler"] = progress_handler

            # FastMCP 2.7.1 的 call_tool 返回 list[TextContent|ImageContent|EmbeddedResource]
            # 而不是 CallToolResult，所以我们需要使用 call_tool_mcp 来获取完整结果
            if hasattr(client, 'call_tool_mcp'):
                # 使用 call_tool_mcp 获取 CallToolResult
                logger.debug(f"Using call_tool_mcp for complete result")
                result = await client.call_tool_mcp(**call_kwargs)

                # 手动处理 raise_on_error 逻辑
                if hasattr(result, 'is_error') and result.is_error and raise_on_error:
                    error_msg = "Tool execution failed"
                    if hasattr(result, 'content') and result.content:
                        for content in result.content:
                            if hasattr(content, 'text'):
                                error_msg = content.text
                                break
                    raise Exception(error_msg)

                return result
            else:
                # 回退到普通的 call_tool
                logger.debug(f"Using standard call_tool")
                content_list = await client.call_tool(**call_kwargs)

                # 将内容列表包装成类似 CallToolResult 的对象
                from types import SimpleNamespace
                result = SimpleNamespace(
                    content=content_list,
                    is_error=False,
                    data=None,
                    structured_content=None
                )

                return result

        except Exception as e:
            logger.error(f"Tool '{tool_name}' execution failed: {e}")
            if raise_on_error:
                raise
            else:
                # 返回错误结果
                from types import SimpleNamespace
                return SimpleNamespace(
                    content=[],
                    is_error=True,
                    data=None,
                    structured_content=None,
                    error=str(e)
                )
    
    def extract_result_data(self, result: 'CallToolResult') -> Any:
        """
        提取结果数据（严格按照 FastMCP 官网标准）

        根据官方文档的优先级顺序：
        1. .data - FastMCP 独有的完全水合 Python 对象
        2. .structured_content - 标准 MCP 结构化 JSON 数据
        3. .content - 标准 MCP 内容块

        Args:
            result: FastMCP 调用结果

        Returns:
            提取的数据
        """
        import logging
        logger = logging.getLogger(__name__)

        # 检查错误状态
        if hasattr(result, 'is_error') and result.is_error:
            logger.warning(f"Tool execution failed, extracting error content")
            # 即使是错误，也尝试提取内容

        # 1. 优先使用 .data 属性（FastMCP 独有特性）
        if hasattr(result, 'data') and result.data is not None:
            logger.debug(f"Using FastMCP .data property: {type(result.data)}")
            return result.data

        # 2. 回退到 .structured_content（标准 MCP 结构化数据）
        if hasattr(result, 'structured_content') and result.structured_content is not None:
            logger.debug(f"Using MCP .structured_content: {result.structured_content}")
            return result.structured_content

        # 3. 最后使用 .content（标准 MCP 内容块）
        if hasattr(result, 'content') and result.content:
            logger.debug(f"Using MCP .content blocks: {len(result.content)} items")

            # 按照官方文档，content 是 ContentBlock 列表
            if isinstance(result.content, list) and result.content:
                # 提取所有内容块的数据
                extracted_content = []

                for content_block in result.content:
                    if hasattr(content_block, 'text'):
                        logger.debug(f"Extracting text from TextContent: {content_block.text}")
                        extracted_content.append(content_block.text)
                    elif hasattr(content_block, 'data'):
                        logger.debug(f"Found binary content: {len(content_block.data)} bytes")
                        extracted_content.append(content_block.data)
                    else:
                        # 对于其他类型的内容块，保留原始对象
                        logger.debug(f"Found other content block type: {type(content_block)}")
                        extracted_content.append(content_block)

                # 根据提取到的内容数量决定返回格式
                if len(extracted_content) == 0:
                    # 没有提取到任何内容，返回第一个原始内容块
                    logger.debug(f"No extractable content found, returning first content block")
                    return result.content[0]
                elif len(extracted_content) == 1:
                    # 只有一个内容块，直接返回内容（保持向后兼容）
                    logger.debug(f"Single content block extracted, returning content directly")
                    return extracted_content[0]
                else:
                    # 多个内容块，返回列表
                    logger.debug(f"Multiple content blocks extracted ({len(extracted_content)}), returning as list")
                    return extracted_content

            # 如果 content 不是列表，直接返回
            return result.content

        # 4. 如果以上都没有数据，返回 None（符合官方文档的 fallback 行为）
        logger.debug("No extractable data found in any standard properties, returning None")
        return None
