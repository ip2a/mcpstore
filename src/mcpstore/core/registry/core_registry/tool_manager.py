"""
Tool Manager - 工具管理模块

负责工具信息的管理和处理，包括：
1. 工具定义的获取和查询
2. 工具信息的格式化和处理
3. JSON Schema 解析和类型推断
4. 工具与服务的关联管理
"""

import logging
from typing import Dict, Any, Optional, List

from .base import ToolManagerInterface
from .utils import JSONSchemaUtils

logger = logging.getLogger(__name__)


class ToolManager(ToolManagerInterface):
    """
    工具管理器实现

    职责：
    - 管理工具定义和信息
    - 处理工具与服务的关联
    - 提供工具查询和过滤功能
    - 处理 JSON Schema 相关逻辑
    """

    def __init__(self, cache_layer, naming_service, namespace: str = "default"):
        super().__init__(cache_layer, naming_service, namespace)

        # 管理器引用（将在后续注入）
        self._relation_manager = None
        self._tool_entity_manager = None
        self._cache_manager = None

        # 工具缓存
        self._tool_cache = {}

        # Schema 处理工具
        self._schema_utils = JSONSchemaUtils()

        self._logger.info(f"[TOOL_MANAGER] [INIT] Initializing ToolManager, namespace: {namespace}")

    def initialize(self) -> None:
        """初始化工具管理器"""
        self._logger.info("[TOOL_MANAGER] [INIT] ToolManager initialization completed")

    def cleanup(self) -> None:
        """清理工具管理器资源"""
        try:
            # 清理缓存
            self._tool_cache.clear()

            # 清理管理器引用
            self._relation_manager = None
            self._tool_entity_manager = None
            self._cache_manager = None

            self._logger.info("[TOOL_MANAGER] [CLEAN] ToolManager cleanup completed")
        except Exception as e:
            self._logger.error(f"[TOOL_MANAGER] [ERROR] ToolManager cleanup error: {e}")
            raise

    def set_managers(self, relation_manager=None, tool_entity_manager=None, cache_manager=None):
        """
        设置依赖的管理器

        Args:
            relation_manager: 关系管理器
            tool_entity_manager: 工具实体管理器
            cache_manager: 缓存管理器
        """
        self._relation_manager = relation_manager
        self._tool_entity_manager = tool_entity_manager
        self._cache_manager = cache_manager
        self._logger.info("[TOOL_MANAGER] [SET] Dependent managers have been set")

    def get_all_tools(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        获取指定agent_id下的所有工具定义

        Args:
            agent_id: Agent ID

        Returns:
            工具定义列表
        """
        try:
            # 使用缓存管理器执行同步操作
            if self._cache_manager:
                tools_dict = self._cache_manager.async_to_sync(
                    self.get_all_tools_dict_async(agent_id),
                    f"list_tools:{agent_id}"
                )
            else:
                # 直接执行异步操作
                import asyncio
                try:
                    loop = asyncio.get_event_loop()
                    if loop.is_running():
                        # 在新线程中运行
                        import concurrent.futures
                        import threading

                        def run_in_thread():
                            new_loop = asyncio.new_event_loop()
                            asyncio.set_event_loop(new_loop)
                            try:
                                return new_loop.run_until_complete(
                                    self.get_all_tools_dict_async(agent_id)
                                )
                            finally:
                                new_loop.close()

                        with concurrent.futures.ThreadPoolExecutor() as executor:
                            future = executor.submit(run_in_thread)
                            tools_dict = future.result()
                    else:
                        tools_dict = loop.run_until_complete(
                            self.get_all_tools_dict_async(agent_id)
                        )
                except RuntimeError:
                    loop = asyncio.new_event_loop()
                    asyncio.set_event_loop(loop)
                    try:
                        tools_dict = loop.run_until_complete(
                            self.get_all_tools_dict_async(agent_id)
                        )
                    finally:
                        loop.close()

            # 转换为列表格式
            # 注意：tool_name 是 tool_global_name，tool_def 中的 "name" 是原始名称
            # 需要确保 "name" 字段使用 tool_global_name
            tools_list = []
            for tool_global_name, tool_def in tools_dict.items():
                tool_entry = dict(tool_def)  # 复制一份，避免修改原始数据
                tool_entry["name"] = tool_global_name  # 确保使用全局名称
                tool_entry["tool_global_name"] = tool_global_name  # 添加全局名称字段
                tools_list.append(tool_entry)

            self._logger.debug(f"[TOOL_MANAGER] [GET] Got all tools: agent={agent_id}, count={len(tools_list)}")
            return tools_list

        except Exception as e:
            self._logger.error(f"[TOOL_MANAGER] [ERROR] Failed to get all tools {agent_id}: {e}")
            return []

    async def get_all_tools_dict_async(self, agent_id: str) -> Dict[str, Dict[str, Any]]:
        """
        从三层缓存架构获取指定Agent的所有工具（异步版本）

        使用新的缓存架构：
        1. 从关系层获取Agent的所有服务
        2. 从关系层获取每个服务的工具列表
        3. 从实体层批量获取工具定义

        Args:
            agent_id: Agent ID

        Returns:
            工具字典 {tool_global_name: tool_definition}
        """
        tools_dict: Dict[str, Dict[str, Any]] = {}

        try:
            # 1. 获取Agent的所有服务关系
            if self._relation_manager:
                services = await self._relation_manager.get_agent_services(agent_id)
            else:
                # 从缓存层直接获取
                services = []

            if not services:
                self._logger.debug(f"[TOOL_MANAGER] [INFO] Agent {agent_id} has no services")
                return tools_dict

            # 2. 收集所有工具全局名称
            all_tool_global_names = []
            for service in services:
                service_global_name = service.get("service_global_name")
                if not service_global_name:
                    continue

                # 获取服务的工具关系
                if self._relation_manager:
                    tool_relations = await self._relation_manager.get_service_tools(
                        service_global_name
                    )

                    for tool_rel in tool_relations:
                        tool_global_name = tool_rel.get("tool_global_name")
                        if tool_global_name:
                            all_tool_global_names.append(tool_global_name)

            if not all_tool_global_names:
                self._logger.debug(f"[TOOL_MANAGER] [INFO] Agent {agent_id} has no tools")
                return tools_dict

            # 3. 批量获取工具实体
            if self._tool_entity_manager:
                tool_entities = await self._tool_entity_manager.get_many_tools(
                    all_tool_global_names
                )
            else:
                # 从缓存层获取
                tool_entities = []

            # 4. 构建工具字典
            for i, entity in enumerate(tool_entities):
                if entity is None:
                    continue

                tool_global_name = all_tool_global_names[i]

                # 转换为标准格式
                tools_dict[tool_global_name] = {
                    "name": entity.tool_original_name,
                    "display_name": entity.tool_original_name,
                    "original_name": entity.tool_original_name,
                    "description": entity.description,
                    "inputSchema": entity.input_schema,
                    "parameters": entity.input_schema,
                    "service_name": entity.service_original_name,
                    "service_global_name": entity.service_global_name,
                    "tool_global_name": entity.tool_global_name,
                    "source_agent": entity.source_agent
                }

            self._logger.debug(f"[TOOL_MANAGER] [GET] Retrieved {len(tools_dict)} tools: agent_id={agent_id}")
            return tools_dict

        except Exception as e:
            self._logger.error(f"[TOOL_MANAGER] [ERROR] Failed to get tools dict asynchronously {agent_id}: {e}")
            return {}

    def list_tools(self, agent_id: str) -> List['ToolInfo']:
        """
        列出工具（返回ToolInfo对象）

        Args:
            agent_id: Agent ID

        Returns:
            ToolInfo对象列表
        """
        try:
            # 获取工具字典
            tools_dict = self.get_all_tools(agent_id)

            # 转换为ToolInfo对象
            tool_infos = []
            for tool_name, tool_def in tools_dict.items():
                tool_info = self._create_tool_info(tool_name, tool_def)
                if tool_info:
                    tool_infos.append(tool_info)

            self._logger.debug(f"[TOOL_MANAGER] [LIST] Listed tools: agent={agent_id}, count={len(tool_infos)}")
            return tool_infos

        except Exception as e:
            self._logger.error(f"[TOOL_MANAGER] [ERROR] Failed to list tools {agent_id}: {e}")
            return []

    def get_all_tool_info(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        获取所有工具的详细信息

        Args:
            agent_id: Agent ID

        Returns:
            工具详细信息列表
        """
        try:
            # 获取工具字典
            tools_dict = self.get_all_tools(agent_id)

            # 生成详细信息
            detailed_tools = []
            for tool_name, tool_def in tools_dict.items():
                detailed_tool = self._get_detailed_tool_info(agent_id, tool_name, tool_def)
                if detailed_tool:
                    detailed_tools.append(detailed_tool)

            self._logger.debug(f"[TOOL_MANAGER] [GET] Got all tool details: agent={agent_id}, count={len(detailed_tools)}")
            return detailed_tools

        except Exception as e:
            self._logger.error(f"[TOOL_MANAGER] [ERROR] Failed to get all tool details {agent_id}: {e}")
            return []

    def get_tools_for_service(self, agent_id: str, service_name: str) -> List[str]:
        """
        获取指定服务的工具列表

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            工具名称列表
        """
        try:
            # 使用缓存管理器执行同步操作
            if self._cache_manager:
                return self._cache_manager.async_to_sync(
                    self.get_tools_for_service_async(agent_id, service_name),
                    f"get_tools_for_service:{agent_id}:{service_name}"
                )
            else:
                # 直接执行异步操作
                import asyncio
                try:
                    loop = asyncio.get_event_loop()
                    if loop.is_running():
                        # 在新线程中运行
                        import concurrent.futures
                        import threading

                        def run_in_thread():
                            new_loop = asyncio.new_event_loop()
                            asyncio.set_event_loop(new_loop)
                            try:
                                return new_loop.run_until_complete(
                                    self.get_tools_for_service_async(agent_id, service_name)
                                )
                            finally:
                                new_loop.close()

                        with concurrent.futures.ThreadPoolExecutor() as executor:
                            future = executor.submit(run_in_thread)
                            return future.result()
                    else:
                        return loop.run_until_complete(
                            self.get_tools_for_service_async(agent_id, service_name)
                        )
                except RuntimeError:
                    loop = asyncio.new_event_loop()
                    asyncio.set_event_loop(loop)
                    try:
                        return loop.run_until_complete(
                            self.get_tools_for_service_async(agent_id, service_name)
                        )
                    finally:
                        loop.close()

        except Exception as e:
            self._logger.error(f"[TOOL_MANAGER] [ERROR] Failed to get service tools {agent_id}:{service_name}: {e}")
            return []

    async def get_tools_for_service_async(self, agent_id: str, service_name: str) -> List[str]:
        """
        异步获取指定服务的工具列表

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            工具名称列表
        """
        try:
            # 生成服务全局名称
            service_global_name = self._naming.generate_service_global_name(service_name, agent_id)

            # 获取服务的工具关系
            if self._relation_manager:
                tool_relations = await self._relation_manager.get_service_tools(service_global_name)
            else:
                tool_relations = []

            # 提取工具全局名称（用于与 tools_dict 匹配）
            tool_names = []
            for tool_rel in tool_relations:
                tool_global_name = tool_rel.get("tool_global_name")
                if tool_global_name:
                    tool_names.append(tool_global_name)

            self._logger.debug(f"[TOOL_MANAGER] [GET] Got service tools: agent={agent_id}, service={service_name}, count={len(tool_names)}")
            return tool_names

        except Exception as e:
            self._logger.error(f"[TOOL_MANAGER] [ERROR] Failed to get service tools asynchronously {agent_id}:{service_name}: {e}")
            return []

    def get_tool_info(self, agent_id: str, tool_name: str) -> Dict[str, Any]:
        """
        获取工具信息

        Args:
            agent_id: Agent ID
            tool_name: 工具名称

        Returns:
            工具信息字典
        """
        try:
            # 检查缓存
            cache_key = f"{agent_id}:{tool_name}"
            if cache_key in self._tool_cache:
                return self._tool_cache[cache_key]

            # 获取工具列表（get_all_tools 返回的是列表，不是字典）
            tools_list = self.get_all_tools(agent_id)

            # 查找指定工具
            for tool_def in tools_list:
                tool_global_name = tool_def.get("name", "")
                tool_original_name = tool_def.get("tool_original_name", "")
                # 匹配工具名称：全局名称、原始名称或名称后缀
                if (tool_global_name == tool_name or 
                    tool_original_name == tool_name or 
                    tool_global_name.endswith(f"_{tool_name}")):
                    detailed_info = self._get_detailed_tool_info(agent_id, tool_name, tool_def)

                    # 更新缓存
                    self._tool_cache[cache_key] = detailed_info

                    return detailed_info

            self._logger.debug(f"[TOOL_MANAGER] [MISS] Tool not found: agent={agent_id}, tool={tool_name}")
            return {}

        except Exception as e:
            self._logger.error(f"Failed to get tool info {agent_id}:{tool_name}: {e}")
            return {}

    def get_session_for_tool(self, agent_id: str, tool_name: str) -> Optional[Any]:
        """
        获取工具对应的会话

        Args:
            agent_id: Agent ID
            tool_name: 工具名称

        Returns:
            会话对象或None
        """
        # 这个方法需要与SessionManager协作
        # 这里返回None，实际实现在主类中委托给SessionManager
        return None

    def _get_detailed_tool_info(self, agent_id: str, tool_name: str, tool_def: Dict[str, Any]) -> Dict[str, Any]:
        """
        获取详细的工具信息

        Args:
            agent_id: Agent ID
            tool_name: 工具名称
            tool_def: 工具定义

        Returns:
            详细工具信息
        """
        try:
            detailed_info = tool_def.copy()

            # 处理输入Schema
            input_schema = tool_def.get("inputSchema", {})
            if input_schema:
                # 处理参数信息
                parameters = {}
                required_params = input_schema.get("required", [])
                properties = input_schema.get("properties", {})

                for param_name, param_info in properties.items():
                    parameters[param_name] = {
                        "type": self._schema_utils.extract_type_from_schema(param_info),
                        "description": self._schema_utils.extract_description_from_schema(param_info),
                        "default": self._schema_utils.get_default_value_from_schema(param_info),
                        "required": param_name in required_params,
                        "enum": param_info.get("enum") if "enum" in param_info else None
                    }

                detailed_info["parameters"] = parameters
                detailed_info["required_params"] = required_params

            # 添加处理后的信息
            detailed_info["formatted_description"] = self._format_tool_description(tool_def)
            detailed_info["parameter_count"] = len(input_schema.get("properties", {}))
            detailed_info["has_parameters"] = bool(input_schema.get("properties"))

            return detailed_info

        except Exception as e:
            self._logger.error(f"Failed to get detailed tool info {agent_id}:{tool_name}: {e}")
            return tool_def.copy()

    def _create_tool_info(self, tool_name: str, tool_def: Dict[str, Any]) -> Optional['ToolInfo']:
        """
        创建ToolInfo对象

        Args:
            tool_name: 工具名称
            tool_def: 工具定义

        Returns:
            ToolInfo对象或None
        """
        try:
            # 这里应该创建实际的ToolInfo对象
            # 由于ToolInfo类的具体定义未知，返回基本信息
            return {
                "name": tool_name,
                "description": tool_def.get("description", ""),
                "input_schema": tool_def.get("inputSchema", {}),
                "service_name": tool_def.get("service_name", "")
            }

        except Exception as e:
            self._logger.error(f"Failed to create ToolInfo {tool_name}: {e}")
            return None

    def _format_tool_description(self, tool_def: Dict[str, Any]) -> str:
        """
        格式化工具描述

        Args:
            tool_def: 工具定义

        Returns:
            格式化的描述
        """
        description = tool_def.get("description", "")
        if not description:
            return "无描述"

        # 基本格式化
        description = description.strip()

        # 限制长度
        if len(description) > 200:
            description = description[:197] + "..."

        return description

    def search_tools(self, agent_id: str, query: str) -> List[Dict[str, Any]]:
        """
        搜索工具

        Args:
            agent_id: Agent ID
            query: 搜索查询

        Returns:
            匹配的工具列表
        """
        try:
            # 获取所有工具
            all_tools = self.get_all_tools(agent_id)

            # 转换为小写进行搜索
            query_lower = query.lower()

            # 搜索匹配的工具
            matched_tools = []
            for tool in all_tools:
                # 搜索工具名称
                tool_name = tool.get("name", "").lower()
                if query_lower in tool_name:
                    matched_tools.append(tool)
                    continue

                # 搜索描述
                description = tool.get("description", "").lower()
                if query_lower in description:
                    matched_tools.append(tool)
                    continue

                # 搜索服务名称
                service_name = tool.get("service_name", "").lower()
                if query_lower in service_name:
                    matched_tools.append(tool)
                    continue

            self._logger.debug(f"Searching tools: agent={agent_id}, query={query}, found={len(matched_tools)}")
            return matched_tools

        except Exception as e:
            self._logger.error(f"Failed to search tools {agent_id}:{query}: {e}")
            return []

    def get_tool_stats(self, agent_id: str) -> Dict[str, Any]:
        """
        获取工具统计信息

        Args:
            agent_id: Agent ID

        Returns:
            统计信息字典
        """
        try:
            # 获取所有工具
            tools = self.get_all_tools(agent_id)

            # 统计信息
            stats = {
                "total_tools": len(tools),
                "tools_with_params": 0,
                "tools_without_params": 0,
                "services": set(),
                "parameter_counts": []
            }

            for tool in tools:
                # 统计参数情况
                input_schema = tool.get("inputSchema", {})
                properties = input_schema.get("properties", {})

                if properties:
                    stats["tools_with_params"] += 1
                    stats["parameter_counts"].append(len(properties))
                else:
                    stats["tools_without_params"] += 1

                # 统计服务
                service_name = tool.get("service_name", "")
                if service_name:
                    stats["services"].add(service_name)

            # 计算平均参数数量
            if stats["parameter_counts"]:
                stats["avg_parameters"] = sum(stats["parameter_counts"]) / len(stats["parameter_counts"])
                stats["max_parameters"] = max(stats["parameter_counts"])
                stats["min_parameters"] = min(stats["parameter_counts"])
            else:
                stats["avg_parameters"] = 0
                stats["max_parameters"] = 0
                stats["min_parameters"] = 0

            # 转换set为list
            stats["services"] = list(stats["services"])
            stats["service_count"] = len(stats["services"])
            del stats["parameter_counts"]

            return stats

        except Exception as e:
            self._logger.error(f"Failed to get tool statistics {agent_id}: {e}")
            return {
                "total_tools": 0,
                "error": str(e)
            }

    def clear_tool_cache(self, agent_id: Optional[str] = None):
        """
        清理工具缓存

        Args:
            agent_id: 可选的agent_id过滤，如果为None则清理所有
        """
        try:
            if agent_id:
                # 清理指定agent的缓存
                keys_to_remove = []
                cache_prefix = f"{agent_id}:"

                for cache_key in self._tool_cache:
                    if cache_key.startswith(cache_prefix):
                        keys_to_remove.append(cache_key)

                for key in keys_to_remove:
                    del self._tool_cache[key]

                self._logger.debug(f"Clearing agent tool cache: {agent_id}")
            else:
                # 清理所有缓存
                self._tool_cache.clear()
                self._logger.debug("Clearing all tool cache")

        except Exception as e:
            self._logger.error(f"Failed to clear tool cache: {e}")

    def get_stats(self) -> Dict[str, Any]:
        """
        获取工具管理器的统计信息

        Returns:
            统计信息字典
        """
        return {
            "namespace": self._namespace,
            "tool_cache_size": len(self._tool_cache),
            "has_relation_manager": self._relation_manager is not None,
            "has_tool_entity_manager": self._tool_entity_manager is not None,
            "has_cache_manager": self._cache_manager is not None
        }
