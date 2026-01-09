#!/usr/bin/env python3
"""
Unified Tool Name Resolver - Based on FastMCP Official Standards
Provides user-friendly tool name input, internally converts to FastMCP standard format
"""

import logging
import re
from dataclasses import dataclass
from typing import Optional, List, Dict, Any

from ..models.tool_result import CallToolFailureResult

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
    Intelligent user-friendly tool name resolver - FastMCP 2.0 standard

    [FEATURES] Core features:
    1. Extremely loose user input: supports any reasonable format
    2. Strict FastMCP standard: fully compliant with official specifications internally
    3. Intelligent unambiguous recognition: automatically handles single/multi-service scenarios
    4. Perfect backward compatibility: maintains existing functionality unchanged

    [SUPPORTED] Input formats:
    - Original tool name: get_current_weather
    - With prefix: mcpstore-demo-weather_get_current_weather
    - Partial match: current_weather, weather
    - Fuzzy match: getcurrentweather, get-current-weather
    """

    def __init__(self, available_services: List[str] = None, is_multi_server: bool = None):
        """
        Initialize intelligent resolver

        Args:
            available_services: List of available services
            is_multi_server: Whether it's a multi-service scenario (None=auto-detect)
        """
        self.available_services = available_services or []
        self.is_multi_server = is_multi_server if is_multi_server is not None else len(self.available_services) > 1
        self._service_tools_cache: Dict[str, List[str]] = {}

        # Preprocess service name mapping
        self._service_name_mapping = {}
        for service in self.available_services:
            normalized = self._normalize_service_name(service)
            self._service_name_mapping[normalized] = service
            self._service_name_mapping[service] = service

        # logger.debug(f"[RESOLVER] init services={len(self.available_services)} multi_server={self.is_multi_server}")
    
    def resolve_tool_name_smart(self, user_input: str, available_tools: List[Dict[str, Any]] = None) -> ToolResolution:
        """
        [SMART] Intelligent user-friendly tool name resolution (new version)

        Supports extremely loose user input, automatically converts to FastMCP standard format:

        Input examples:
        - "get_current_weather" → Auto-detect service and add prefix (multi-service)
        - "mcpstore-demo-weather_get_current_weather" → Parse and validate
        - "weather" → Intelligently match most similar tool
        - "getcurrentweather" → Fuzzy match and suggest

        Args:
            user_input: User input tool name (any format)
            available_tools: List of available tools

        Returns:
            ToolResolution: Resolution result containing FastMCP standard format
        """
        if not user_input or not isinstance(user_input, str):
            raise ValueError("Tool name cannot be empty")

        user_input = user_input.strip()
        logger.debug(f"[SMART_RESOLVE] start input='{user_input}' multi_server={self.is_multi_server}")

        # Build tool mapping table
        tool_mappings = self._build_smart_tool_mappings(available_tools or [])

        # Smart resolution process
        resolution = None

        # 1. Exact match (highest priority)
        resolution = self._try_exact_match(user_input, tool_mappings)
        if resolution:
            logger.debug(f"[EXACT_MATCH] {user_input} -> {resolution.service_name}::{resolution.original_tool_name}")
            return resolution

        # 2. Smart prefix match
        resolution = self._try_prefix_match(user_input, tool_mappings)
        if resolution:
            logger.debug(f"[PREFIX_MATCH] {user_input} -> {resolution.service_name}::{resolution.original_tool_name}")
            return resolution

        # 3. No prefix smart match (single service optimization)
        resolution = self._try_no_prefix_match(user_input, tool_mappings)
        if resolution:
            logger.debug(f"[NO_PREFIX_MATCH] {user_input} -> {resolution.service_name}::{resolution.original_tool_name}")
            return resolution

        # 4. Smart fuzzy match
        resolution = self._try_fuzzy_match(user_input, tool_mappings)
        if resolution:
            logger.debug(f"[FUZZY_MATCH] {user_input} -> {resolution.service_name}::{resolution.original_tool_name}")
            return resolution

        # 5. Failure handling: provide smart suggestions
        suggestions = self._get_smart_suggestions(user_input, tool_mappings)
        if suggestions:
            raise ValueError(f"Tool '{user_input}' not found. Did you mean: {', '.join(suggestions[:3])}?")
        else:
            raise ValueError(f"Tool '{user_input}' not found and no similar suggestions available")

    def resolve_tool_name(self, user_input: str, available_tools: List[Dict[str, Any]] = None) -> ToolResolution:
        """
        Resolve user-input tool name

        Args:
            user_input: User-input tool name
            available_tools: Available tools list [{"name": "display_name", "original_name": "tool", "service_name": "service"}]

        Returns:
            ToolResolution: Resolution result

        Raises:
            ValueError: Cannot resolve tool name
        """
        if not user_input or not isinstance(user_input, str):
            raise ValueError("Tool name cannot be empty")

        user_input = user_input.strip()
        available_tools = available_tools or []

        # Build tool mapping (support display names and original names)
        display_to_original = {}  # display_name -> (original_name, service_name)
        original_to_service = {}  # original_name -> service_name
        service_tools = {}        # service_name -> [original_tool_name_list]

        for tool in available_tools:
            display_name = tool.get("name", "")  # display name
            original_name = tool.get("original_name") or tool.get("name", "")  # original name
            service_name = tool.get("service_name", "")

            display_to_original[display_name] = (original_name, service_name)
            original_to_service[original_name] = service_name

            if service_name not in service_tools:
                service_tools[service_name] = []
            if original_name not in service_tools[service_name]:
                service_tools[service_name].append(original_name)

        logger.debug(f"Resolving tool: {user_input}")
        logger.debug(f"Available services: {list(service_tools.keys())}")

        # 1. Exact match: display name
        if user_input in display_to_original:
            original_name, service_name = display_to_original[user_input]
            return ToolResolution(
                service_name=service_name,
                original_tool_name=original_name,
                user_input=user_input,
                resolution_method="exact_display_match"
            )

        # 2. Exact match: original name
        if user_input in original_to_service:
            return ToolResolution(
                service_name=original_to_service[user_input],
                original_tool_name=user_input,
                user_input=user_input,
                resolution_method="exact_original_match"
            )

        # 3. Single underscore format parsing: service_tool (exact service name match)
        if "_" in user_input and "__" not in user_input:
            # Try all possible split points
            for i in range(1, len(user_input)):
                if user_input[i] == "_":
                    potential_service = user_input[:i]
                    potential_tool = user_input[i+1:]

                    # Check if there's a matching service (support original names and normalized names)
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

        # 4. Check if deprecated double underscore format is used
        if "__" in user_input:
            parts = user_input.split("__", 1)
            if len(parts) == 2:
                potential_service, potential_tool = parts
                single_underscore_format = f"{potential_service}_{potential_tool}"
                raise ValueError(
                    f"Double underscore format '__' is no longer supported. "
                    f"Please use single underscore format: '{single_underscore_format}'"
                )

        # 5. Fuzzy match: find similar names in all tools
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
            # Multiple matches, provide suggestions
            suggestions = [display_name for _, _, display_name in fuzzy_matches[:3]]
            raise ValueError(f"Ambiguous tool name '{user_input}'. Did you mean: {', '.join(suggestions)}?")

        # 6. Cannot resolve, provide suggestions
        if available_tools:
            all_display_names = list(display_to_original.keys())
            suggestions = self._get_suggestions(user_input, all_display_names)
            if suggestions:
                raise ValueError(f"Tool '{user_input}' not found. Did you mean: {', '.join(suggestions[:3])}?")

        raise ValueError(f"Tool '{user_input}' not found")
    
    def create_user_friendly_name(self, service_name: str, tool_name: str) -> str:
        """
        Create user-friendly tool name (for display)

        Uses single underscore format, keeping service name in original form

        Args:
            service_name: Service name (keep original format)
            tool_name: Original tool name

        Returns:
            User-friendly tool name
        """
        # Use single underscore, keep service name in original format
        return f"{service_name}_{tool_name}"
    
    def _normalize_service_name(self, service_name: str) -> str:
        """Normalize service name"""
        # Remove special characters, convert to underscores
        normalized = re.sub(r'[^a-zA-Z0-9_]', '_', service_name)
        # Remove consecutive underscores
        normalized = re.sub(r'_+', '_', normalized)
        # Remove leading and trailing underscores
        normalized = normalized.strip('_')
        return normalized or "unnamed"

    def _is_fuzzy_match(self, user_input: str, tool_name: str) -> bool:
        """Check if it's a fuzzy match"""
        user_lower = user_input.lower()
        tool_lower = tool_name.lower()

        # Complete containment
        if user_lower in tool_lower or tool_lower in user_lower:
            return True

        # Match after removing underscores
        user_clean = user_lower.replace('_', '').replace('-', '')
        tool_clean = tool_lower.replace('_', '').replace('-', '')

        if user_clean in tool_clean or tool_clean in user_clean:
            return True

        return False
    
    def _get_suggestions(self, user_input: str, available_names: List[str]) -> List[str]:
        """Get suggested tool names"""
        suggestions = []
        user_lower = user_input.lower()

        for name in available_names:
            name_lower = name.lower()
            # Prefix match
            if name_lower.startswith(user_lower) or user_lower.startswith(name_lower):
                suggestions.append(name)
            # Containment match
            elif user_lower in name_lower or name_lower in user_lower:
                suggestions.append(name)

        return sorted(suggestions, key=lambda x: len(x))[:5]

    def _build_smart_tool_mappings(self, available_tools: List[Dict[str, Any]]) -> Dict[str, Any]:
        """
        Build smart tool mapping table

        Returns:
            Dictionary containing multiple mapping relationships:
            - exact_matches: Exact match mapping
            - prefix_matches: Prefix match mapping
            - no_prefix_matches: No prefix match mapping
            - fuzzy_candidates: Fuzzy match candidates
        """
        mappings = {
            "exact_matches": {},      # {user_input: (service, original_tool)}
            "prefix_matches": {},     # {prefix_removed: [(service, original_tool, full_name)]}
            "no_prefix_matches": {},  # {tool_name: [(service, original_tool, full_name)]}
            "fuzzy_candidates": [],   # [(service, original_tool, full_name, display_name)]
            "all_tools": []          # Complete information for all tools
        }

        for tool in available_tools:
            service_name = tool.get("service_name", "")
            original_name = tool.get("original_name", "")
            display_name = tool.get("name", "")

            if not service_name or not original_name:
                continue

            # Record all tools
            tool_info = (service_name, original_name, display_name)
            mappings["all_tools"].append(tool_info)
            mappings["fuzzy_candidates"].append(tool_info + (display_name,))

            # Exact matches: display names and original names
            mappings["exact_matches"][display_name] = (service_name, original_name)
            mappings["exact_matches"][original_name] = (service_name, original_name)

            # Prefix matches: tool name after removing service name prefix
            if display_name.startswith(f"{service_name}_"):
                tool_suffix = display_name[len(service_name) + 1:]
                if tool_suffix not in mappings["prefix_matches"]:
                    mappings["prefix_matches"][tool_suffix] = []
                mappings["prefix_matches"][tool_suffix].append((service_name, original_name, display_name))

            # No prefix matches: pure tool name
            if original_name not in mappings["no_prefix_matches"]:
                mappings["no_prefix_matches"][original_name] = []
            mappings["no_prefix_matches"][original_name].append((service_name, original_name, display_name))

        logger.debug(f"[MAPPINGS] built exact={len(mappings['exact_matches'])} prefix={len(mappings['prefix_matches'])} no_prefix={len(mappings['no_prefix_matches'])}")
        return mappings

    def _try_exact_match(self, user_input: str, mappings: Dict[str, Any]) -> Optional[ToolResolution]:
        """Try exact match"""
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
        """Try prefix match: user input contains service name prefix"""
        # Check if it contains service name prefix
        for service_name in self.available_services:
            if user_input.startswith(f"{service_name}_"):
                tool_suffix = user_input[len(service_name) + 1:]
                if tool_suffix in mappings["prefix_matches"]:
                    candidates = mappings["prefix_matches"][tool_suffix]
                    # Prioritize matching tools from the same service
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
        """Try no prefix match: user input does not contain service name prefix"""
        if user_input in mappings["no_prefix_matches"]:
            candidates = mappings["no_prefix_matches"][user_input]

            if len(candidates) == 1:
                # Unique match
                service_name, original_name, display_name = candidates[0]
                return ToolResolution(
                    service_name=service_name,
                    original_tool_name=original_name,
                    user_input=user_input,
                    resolution_method="no_prefix_match"
                )
            elif len(candidates) > 1:
                # Multiple matches, select first in single service mode, error in multi service mode
                if not self.is_multi_server:
                    service_name, original_name, display_name = candidates[0]
                    return ToolResolution(
                        service_name=service_name,
                        original_tool_name=original_name,
                        user_input=user_input,
                        resolution_method="no_prefix_match_single_server"
                    )
                else:
                    # Ambiguous in multi service mode, return None for subsequent processing
                    logger.debug(f"[NO_PREFIX] ambiguous user_input='{user_input}' candidates={len(candidates)}")
        return None

    def _try_fuzzy_match(self, user_input: str, mappings: Dict[str, Any]) -> Optional[ToolResolution]:
        """Try fuzzy match: smart similarity matching"""
        fuzzy_matches = []
        user_clean = self._clean_for_fuzzy_match(user_input)

        for service_name, original_name, display_name, _ in mappings["fuzzy_candidates"]:
            # Check fuzzy matching of display names and original names
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
        """Get smart suggestions"""
        suggestions = []
        user_lower = user_input.lower()
        user_clean = self._clean_for_fuzzy_match(user_input)

        # Collect all possible suggestions
        candidates = []
        for service_name, original_name, display_name, _ in mappings["fuzzy_candidates"]:
            score = self._calculate_similarity_score(user_clean, display_name, original_name)
            if score > 0:
                candidates.append((score, display_name))

        # Sort by similarity and return top few
        candidates.sort(key=lambda x: x[0], reverse=True)
        return [name for score, name in candidates[:5] if score > 0.3]

    def _clean_for_fuzzy_match(self, text: str) -> str:
        """Clean text for fuzzy matching"""
        return re.sub(r'[^a-zA-Z0-9]', '', text.lower())

    def _is_smart_fuzzy_match(self, user_clean: str, target: str) -> bool:
        """Smart fuzzy match judgment"""
        target_clean = self._clean_for_fuzzy_match(target)

        # Complete containment
        if user_clean in target_clean or target_clean in user_clean:
            return True

        # Prefix match (at least 3 characters)
        if len(user_clean) >= 3 and (target_clean.startswith(user_clean) or user_clean.startswith(target_clean)):
            return True

        return False

    def _calculate_similarity_score(self, user_clean: str, display_name: str, original_name: str) -> float:
        """Calculate similarity score"""
        display_clean = self._clean_for_fuzzy_match(display_name)
        original_clean = self._clean_for_fuzzy_match(original_name)

        max_score = 0.0

        # Check display name
        if user_clean == display_clean:
            max_score = max(max_score, 1.0)
        elif user_clean in display_clean:
            max_score = max(max_score, 0.8)
        elif display_clean.startswith(user_clean) or user_clean.startswith(display_clean):
            max_score = max(max_score, 0.6)

        # Check original name
        if user_clean == original_clean:
            max_score = max(max_score, 1.0)
        elif user_clean in original_clean:
            max_score = max(max_score, 0.8)
        elif original_clean.startswith(user_clean) or user_clean.startswith(original_clean):
            max_score = max(max_score, 0.6)

        return max_score

    def to_fastmcp_format(self, resolution: ToolResolution, available_tools: List[Dict[str, Any]] = None) -> str:
        """
        Convert to FastMCP standard format tool name

         Important discovery:
        - MCPStore internal: tool names with prefix "mcpstore-demo-weather_get_current_weather"
        - FastMCP native: tool names without prefix "get_current_weather"
        - We need to return the format expected by FastMCP native!

        Args:
            resolution: Tool resolution result
            available_tools: Available tools list (for finding original names)

        Returns:
            Tool name expected by FastMCP native (original name without prefix)
        """
        # Key correction: FastMCP execution needs original tool name, not MCPStore internal prefixed name
        logger.debug(f"[FASTMCP] native_tool_name={resolution.original_tool_name}")
        return resolution.original_tool_name

    def resolve_and_format_for_fastmcp(self, user_input: str, available_tools: List[Dict[str, Any]] = None) -> tuple[str, ToolResolution]:
        """
        One-stop resolution: user input → FastMCP standard format

        This is the main external interface, completing the full conversion from user-friendly input to FastMCP standard format

        Args:
            user_input: User-input tool name (any format)
            available_tools: Available tools list

        Returns:
            tuple: (fastmcp_format_name, resolution_details)
        """
        # 1. Smart resolution of user input
        resolution = self.resolve_tool_name_smart(user_input, available_tools)

        # 2. Convert to FastMCP standard format (pass available_tools for finding actual names)
        fastmcp_name = self.to_fastmcp_format(resolution, available_tools)

        logger.info(f"[RESOLVE_SUCCESS] input='{user_input}' fastmcp='{fastmcp_name}' service='{resolution.service_name}' method='{resolution.resolution_method}'")

        return fastmcp_name, resolution

class FastMCPToolExecutor:
    """
    FastMCP standard tool executor
    Strictly executes tool calls according to official website standards
    """

    def __init__(self, default_timeout: float = 30.0):
        """
        Initialize executor

        Args:
            default_timeout: Default timeout time (seconds)
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
        Execute tool (strictly according to FastMCP official website standards)

        Only use FastMCP official client's call_tool return object, without any custom "equivalent object" wrapping,
        no longer fallback to call_tool_mcp for field mapping, ensuring result format matches official standards.

        Args:
            client: FastMCP client instance (must implement call_tool)
            tool_name: Tool name (FastMCP original name)
            arguments: Tool parameters
            timeout: Timeout time (seconds)
            progress_handler: Progress handler
            raise_on_error: Whether to raise exception on error

        Returns:
            CallToolResult: FastMCP standard result object
        """
        arguments = arguments or {}
        timeout = timeout or self.default_timeout

        try:
            if not hasattr(client, 'call_tool'):
                raise RuntimeError("FastMCP client does not support call_tool; please use a compatible FastMCP client")

            logger.debug("Using client.call_tool (FastMCP official) for result")
            result = await client.call_tool(
                name=tool_name,
                arguments=arguments,
                timeout=timeout,
                progress_handler=progress_handler,
                raise_on_error=raise_on_error,
            )
            return result

        except Exception as e:
            logger.error(f"Tool '{tool_name}' execution failed: {e}")
            if raise_on_error:
                raise
            failure = CallToolFailureResult(str(e))
            return failure.unwrap()
    
    def extract_result_data(self, result: 'CallToolResult') -> Any:
        """
        Extract result data (strictly according to FastMCP official website standards)

        Priority order according to official documentation:
        1. .data - FastMCP unique fully hydrated Python object
        2. .structured_content - Standard MCP structured JSON data
        3. .content - Standard MCP content blocks

        Args:
            result: FastMCP call result

        Returns:
            Extracted data
        """
        import logging
        logger = logging.getLogger(__name__)

        # Check error status
        if hasattr(result, 'is_error') and result.is_error:
            logger.warning(f"Tool execution failed, extracting error content")
            # Even for errors, try to extract content

        # 1. Prioritize .data property (FastMCP unique feature)
        if hasattr(result, 'data') and result.data is not None:
            logger.debug(f"Using FastMCP .data property: {type(result.data)}")
            return result.data

        # 2. Fallback to .structured_content (standard MCP structured data)
        if hasattr(result, 'structured_content') and result.structured_content is not None:
            logger.debug(f"Using MCP .structured_content: {result.structured_content}")
            return result.structured_content

        # 3. Finally use .content (standard MCP content blocks)
        if hasattr(result, 'content') and result.content:
            logger.debug(f"Using MCP .content blocks: {len(result.content)} items")

            # According to official documentation, content is a ContentBlock list
            if isinstance(result.content, list) and result.content:
                # Extract data from all content blocks
                extracted_content = []

                for content_block in result.content:
                    if hasattr(content_block, 'text'):
                        logger.debug(f"Extracting text from TextContent: {content_block.text}")
                        extracted_content.append(content_block.text)
                    elif hasattr(content_block, 'data'):
                        logger.debug(f"Found binary content: {len(content_block.data)} bytes")
                        extracted_content.append(content_block.data)
                    else:
                        # For other types of content blocks, keep original object
                        logger.debug(f"Found other content block type: {type(content_block)}")
                        extracted_content.append(content_block)

                # Decide return format based on extracted content count
                if len(extracted_content) == 0:
                    # No extractable content, return first original content block
                    logger.debug(f"No extractable content found, returning first content block")
                    return result.content[0]
                elif len(extracted_content) == 1:
                    # Only one content block, return content directly (maintain backward compatibility)
                    logger.debug(f"Single content block extracted, returning content directly")
                    return extracted_content[0]
                else:
                    # Multiple content blocks, return list
                    logger.debug(f"Multiple content blocks extracted ({len(extracted_content)}), returning as list")
                    return extracted_content

            # If content is not a list, return directly
            return result.content

        # 4. If no data from above, return None (matches official documentation fallback behavior)
        logger.debug("No extractable data found in any standard properties, returning None")
        return None
