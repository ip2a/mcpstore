#!/usr/bin/env python3
"""
Unified Tool Name Resolver - Based on FastMCP Official Standards
Provides user-friendly tool name input, internally converts to FastMCP standard format
"""

import logging
import re
from dataclasses import dataclass
from typing import Optional, List, Dict, Any

logger = logging.getLogger(__name__)

@dataclass
class ToolResolution:
    """Tool resolution result"""
    service_name: str           # Service name
    original_tool_name: str     # FastMCP standard original tool name
    user_input: str            # User input tool name
    resolution_method: str     # Resolution method (exact_match, prefix_match, fuzzy_match)

class ToolNameResolver:
    """
    智能用户友好型工具名称解析器 - FastMCP 2.0 标准

    🎯 核心特性：
    1. 极度宽松的用户输入：支持任何合理格式
    2. 严格的FastMCP标准：内部完全符合官网规范
    3. 智能无歧义识别：自动处理单/多服务场景
    4. 完美向后兼容：保持现有功能不变

    📝 支持的输入格式：
    - 原始工具名：get_current_weather
    - 带前缀：mcpstore-demo-weather_get_current_weather
    - 部分匹配：current_weather, weather
    - 模糊匹配：getcurrentweather, get-current-weather
    """

    def __init__(self, available_services: List[str] = None, is_multi_server: bool = None):
        """
        初始化智能解析器

        Args:
            available_services: 可用服务列表
            is_multi_server: 是否为多服务场景（None=自动检测）
        """
        self.available_services = available_services or []
        self.is_multi_server = is_multi_server if is_multi_server is not None else len(self.available_services) > 1
        self._service_tools_cache: Dict[str, List[str]] = {}

        # 预处理服务名映射
        self._service_name_mapping = {}
        for service in self.available_services:
            normalized = self._normalize_service_name(service)
            self._service_name_mapping[normalized] = service
            self._service_name_mapping[service] = service

        # logger.debug(f"[RESOLVER] init services={len(self.available_services)} multi_server={self.is_multi_server}")
    
    def resolve_tool_name_smart(self, user_input: str, available_tools: List[Dict[str, Any]] = None) -> ToolResolution:
        """
        🚀 智能用户友好型工具名称解析（新版本）

        支持极度宽松的用户输入，自动转换为FastMCP标准格式：

        输入示例：
        - "get_current_weather" → 自动识别服务并添加前缀（多服务时）
        - "mcpstore-demo-weather_get_current_weather" → 解析并验证
        - "weather" → 智能匹配最相似的工具
        - "getcurrentweather" → 模糊匹配并建议

        Args:
            user_input: 用户输入的工具名称（任何格式）
            available_tools: 可用工具列表

        Returns:
            ToolResolution: 包含FastMCP标准格式的解析结果
        """
        if not user_input or not isinstance(user_input, str):
            raise ValueError("工具名称不能为空")

        user_input = user_input.strip()
        logger.debug(f"[SMART_RESOLVE] start input='{user_input}' multi_server={self.is_multi_server}")

        # 构建工具映射表
        tool_mappings = self._build_smart_tool_mappings(available_tools or [])

        # 🎯 智能解析流程
        resolution = None

        # 1. 精确匹配（最高优先级）
        resolution = self._try_exact_match(user_input, tool_mappings)
        if resolution:
            logger.debug(f"[EXACT_MATCH] {user_input} -> {resolution.service_name}::{resolution.original_tool_name}")
            return resolution

        # 2. 前缀智能匹配
        resolution = self._try_prefix_match(user_input, tool_mappings)
        if resolution:
            logger.debug(f"[PREFIX_MATCH] {user_input} -> {resolution.service_name}::{resolution.original_tool_name}")
            return resolution

        # 3. 无前缀智能匹配（单服务优化）
        resolution = self._try_no_prefix_match(user_input, tool_mappings)
        if resolution:
            logger.debug(f"[NO_PREFIX_MATCH] {user_input} -> {resolution.service_name}::{resolution.original_tool_name}")
            return resolution

        # 4. 模糊智能匹配
        resolution = self._try_fuzzy_match(user_input, tool_mappings)
        if resolution:
            logger.debug(f"[FUZZY_MATCH] {user_input} -> {resolution.service_name}::{resolution.original_tool_name}")
            return resolution

        # 5. 失败处理：提供智能建议
        suggestions = self._get_smart_suggestions(user_input, tool_mappings)
        if suggestions:
            raise ValueError(f"工具 '{user_input}' 未找到。你是否想要: {', '.join(suggestions[:3])}?")
        else:
            raise ValueError(f"工具 '{user_input}' 未找到，且无相似建议")

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

    def _build_smart_tool_mappings(self, available_tools: List[Dict[str, Any]]) -> Dict[str, Any]:
        """
        构建智能工具映射表

        Returns:
            包含多种映射关系的字典：
            - exact_matches: 精确匹配映射
            - prefix_matches: 前缀匹配映射
            - no_prefix_matches: 无前缀匹配映射
            - fuzzy_candidates: 模糊匹配候选
        """
        mappings = {
            "exact_matches": {},      # {user_input: (service, original_tool)}
            "prefix_matches": {},     # {prefix_removed: [(service, original_tool, full_name)]}
            "no_prefix_matches": {},  # {tool_name: [(service, original_tool, full_name)]}
            "fuzzy_candidates": [],   # [(service, original_tool, full_name, display_name)]
            "all_tools": []          # 所有工具的完整信息
        }

        for tool in available_tools:
            service_name = tool.get("service_name", "")
            original_name = tool.get("original_name", "")
            display_name = tool.get("name", "")

            if not service_name or not original_name:
                continue

            # 记录所有工具
            tool_info = (service_name, original_name, display_name)
            mappings["all_tools"].append(tool_info)
            mappings["fuzzy_candidates"].append(tool_info + (display_name,))

            # 精确匹配：显示名称和原始名称
            mappings["exact_matches"][display_name] = (service_name, original_name)
            mappings["exact_matches"][original_name] = (service_name, original_name)

            # 前缀匹配：移除服务名前缀后的工具名
            if display_name.startswith(f"{service_name}_"):
                tool_suffix = display_name[len(service_name) + 1:]
                if tool_suffix not in mappings["prefix_matches"]:
                    mappings["prefix_matches"][tool_suffix] = []
                mappings["prefix_matches"][tool_suffix].append((service_name, original_name, display_name))

            # 无前缀匹配：纯工具名
            if original_name not in mappings["no_prefix_matches"]:
                mappings["no_prefix_matches"][original_name] = []
            mappings["no_prefix_matches"][original_name].append((service_name, original_name, display_name))

        logger.debug(f"[MAPPINGS] built exact={len(mappings['exact_matches'])} prefix={len(mappings['prefix_matches'])} no_prefix={len(mappings['no_prefix_matches'])}")
        return mappings

    def _try_exact_match(self, user_input: str, mappings: Dict[str, Any]) -> Optional[ToolResolution]:
        """尝试精确匹配"""
        if user_input in mappings["exact_matches"]:
            service_name, original_name = mappings["exact_matches"][user_input]
            return ToolResolution(
                service_name=service_name,
                original_tool_name=original_name,
                user_input=user_input,
                resolution_method="exact_match"
            )
        return None

    def _try_prefix_match(self, user_input: str, mappings: Dict[str, Any]) -> Optional[ToolResolution]:
        """尝试前缀匹配：用户输入包含服务名前缀"""
        # 检查是否包含服务名前缀
        for service_name in self.available_services:
            if user_input.startswith(f"{service_name}_"):
                tool_suffix = user_input[len(service_name) + 1:]
                if tool_suffix in mappings["prefix_matches"]:
                    candidates = mappings["prefix_matches"][tool_suffix]
                    # 优先匹配相同服务的工具
                    for candidate_service, original_name, display_name in candidates:
                        if candidate_service == service_name:
                            return ToolResolution(
                                service_name=candidate_service,
                                original_tool_name=original_name,
                                user_input=user_input,
                                resolution_method="prefix_match"
                            )
        return None

    def _try_no_prefix_match(self, user_input: str, mappings: Dict[str, Any]) -> Optional[ToolResolution]:
        """尝试无前缀匹配：用户输入不包含服务名前缀"""
        if user_input in mappings["no_prefix_matches"]:
            candidates = mappings["no_prefix_matches"][user_input]

            if len(candidates) == 1:
                # 唯一匹配
                service_name, original_name, display_name = candidates[0]
                return ToolResolution(
                    service_name=service_name,
                    original_tool_name=original_name,
                    user_input=user_input,
                    resolution_method="no_prefix_match"
                )
            elif len(candidates) > 1:
                # 多个匹配，在单服务模式下选择第一个，多服务模式下报错
                if not self.is_multi_server:
                    service_name, original_name, display_name = candidates[0]
                    return ToolResolution(
                        service_name=service_name,
                        original_tool_name=original_name,
                        user_input=user_input,
                        resolution_method="no_prefix_match_single_server"
                    )
                else:
                    # 多服务模式下有歧义，返回None让后续处理
                    logger.debug(f"[NO_PREFIX] ambiguous user_input='{user_input}' candidates={len(candidates)}")
        return None

    def _try_fuzzy_match(self, user_input: str, mappings: Dict[str, Any]) -> Optional[ToolResolution]:
        """尝试模糊匹配：智能相似度匹配"""
        fuzzy_matches = []
        user_clean = self._clean_for_fuzzy_match(user_input)

        for service_name, original_name, display_name, _ in mappings["fuzzy_candidates"]:
            # 检查显示名称和原始名称的模糊匹配
            if self._is_smart_fuzzy_match(user_clean, display_name) or \
               self._is_smart_fuzzy_match(user_clean, original_name):
                fuzzy_matches.append((service_name, original_name, display_name))

        if len(fuzzy_matches) == 1:
            service_name, original_name, display_name = fuzzy_matches[0]
            return ToolResolution(
                service_name=service_name,
                original_tool_name=original_name,
                user_input=user_input,
                resolution_method="fuzzy_match"
            )
        elif len(fuzzy_matches) > 1:
            logger.debug(f"[FUZZY] multiple_matches input='{user_input}' count={len(fuzzy_matches)}")

        return None

    def _get_smart_suggestions(self, user_input: str, mappings: Dict[str, Any]) -> List[str]:
        """获取智能建议"""
        suggestions = []
        user_lower = user_input.lower()
        user_clean = self._clean_for_fuzzy_match(user_input)

        # 收集所有可能的建议
        candidates = []
        for service_name, original_name, display_name, _ in mappings["fuzzy_candidates"]:
            score = self._calculate_similarity_score(user_clean, display_name, original_name)
            if score > 0:
                candidates.append((score, display_name))

        # 按相似度排序并返回前几个
        candidates.sort(key=lambda x: x[0], reverse=True)
        return [name for score, name in candidates[:5] if score > 0.3]

    def _clean_for_fuzzy_match(self, text: str) -> str:
        """清理文本用于模糊匹配"""
        return re.sub(r'[^a-zA-Z0-9]', '', text.lower())

    def _is_smart_fuzzy_match(self, user_clean: str, target: str) -> bool:
        """智能模糊匹配判断"""
        target_clean = self._clean_for_fuzzy_match(target)

        # 完全包含
        if user_clean in target_clean or target_clean in user_clean:
            return True

        # 前缀匹配（至少3个字符）
        if len(user_clean) >= 3 and (target_clean.startswith(user_clean) or user_clean.startswith(target_clean)):
            return True

        return False

    def _calculate_similarity_score(self, user_clean: str, display_name: str, original_name: str) -> float:
        """计算相似度分数"""
        display_clean = self._clean_for_fuzzy_match(display_name)
        original_clean = self._clean_for_fuzzy_match(original_name)

        max_score = 0.0

        # 检查显示名称
        if user_clean == display_clean:
            max_score = max(max_score, 1.0)
        elif user_clean in display_clean:
            max_score = max(max_score, 0.8)
        elif display_clean.startswith(user_clean) or user_clean.startswith(display_clean):
            max_score = max(max_score, 0.6)

        # 检查原始名称
        if user_clean == original_clean:
            max_score = max(max_score, 1.0)
        elif user_clean in original_clean:
            max_score = max(max_score, 0.8)
        elif original_clean.startswith(user_clean) or user_clean.startswith(original_clean):
            max_score = max(max_score, 0.6)

        return max_score

    def to_fastmcp_format(self, resolution: ToolResolution, available_tools: List[Dict[str, Any]] = None) -> str:
        """
        转换为FastMCP标准格式的工具名称

        🔧 重要发现：
        - MCPStore内部：工具名称带前缀 "mcpstore-demo-weather_get_current_weather"
        - FastMCP原生：工具名称不带前缀 "get_current_weather"
        - 我们需要返回FastMCP原生期望的格式！

        Args:
            resolution: 工具解析结果
            available_tools: 可用工具列表（用于查找原始名称）

        Returns:
            FastMCP原生期望的工具名称（不带前缀的原始名称）
        """
        # 关键修正：FastMCP执行时需要原始工具名称，不是MCPStore内部的带前缀名称
        logger.debug(f"[FASTMCP] native_tool_name={resolution.original_tool_name}")
        return resolution.original_tool_name

    def resolve_and_format_for_fastmcp(self, user_input: str, available_tools: List[Dict[str, Any]] = None) -> tuple[str, ToolResolution]:
        """
        🚀 一站式解析：用户输入 → FastMCP标准格式

        这是对外的主要接口，完成从用户友好输入到FastMCP标准格式的完整转换

        Args:
            user_input: 用户输入的工具名称（任何格式）
            available_tools: 可用工具列表

        Returns:
            tuple: (fastmcp_format_name, resolution_details)
        """
        # 1. 智能解析用户输入
        resolution = self.resolve_tool_name_smart(user_input, available_tools)

        # 2. 转换为FastMCP标准格式（传入available_tools用于查找实际名称）
        fastmcp_name = self.to_fastmcp_format(resolution, available_tools)

        logger.info(f"[RESOLVE_SUCCESS] input='{user_input}' fastmcp='{fastmcp_name}' service='{resolution.service_name}' method='{resolution.resolution_method}'")

        return fastmcp_name, resolution

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
