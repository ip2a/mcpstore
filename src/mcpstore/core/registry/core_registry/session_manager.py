"""
Session Manager - 会话管理模块

负责管理MCP服务的会话对象，包括：
1. 服务会话的存储和检索
2. 工具到会话的映射管理
3. 会话的生命周期管理
4. 会话数据的内存隔离

注意：会话数据总是存储在内存中，因为MCP Session对象不可序列化。
"""

import logging
from typing import Dict, Any, Optional, List

from .base import SessionManagerInterface

logger = logging.getLogger(__name__)


class SessionManager(SessionManagerInterface):
    """
    会话管理器实现

    职责：
    - 管理服务会话对象（内存存储）
    - 维护工具到会话的映射关系
    - 提供会话的增删改查操作
    - 确保会话数据的agent隔离
    """

    def __init__(self, cache_layer, naming_service, namespace: str = "default"):
        super().__init__(cache_layer, naming_service, namespace)

        # 服务会话存储 - agent_id: {service_name: session}
        # 注意：会话数据总是存储在内存中，因为MCP Session对象不可序列化
        self.sessions: Dict[str, Dict[str, Any]] = {}

        # 工具到会话的映射 - agent_id: {tool_name: session}
        self.tool_to_session_map: Dict[str, Dict[str, Any]] = {}

        self._logger.info(f"[SESSION_MANAGER] [INIT] Initializing SessionManager, namespace: {namespace}")

    def initialize(self) -> None:
        """初始化会话管理器"""
        self._logger.info("[SESSION_MANAGER] [INIT] SessionManager initialization completed")

    def cleanup(self) -> None:
        """清理会话管理器资源"""
        try:
            # 清理所有会话数据
            session_count = sum(len(services) for services in self.sessions.values())
            tool_mapping_count = sum(len(tools) for tools in self.tool_to_session_map.values())

            self.sessions.clear()
            self.tool_to_session_map.clear()

            self._logger.info(f"[SESSION_MANAGER] [CLEAN] SessionManager cleanup completed: cleared {session_count} service sessions, {tool_mapping_count} tool mappings")
        except Exception as e:
            self._logger.error(f"[SESSION_MANAGER] [ERROR] SessionManager cleanup error: {e}")
            raise

    def get_session(self, agent_id: str, name: str) -> Optional[Any]:
        """
        获取指定agent_id下服务的会话对象（同步，仅内存）

        Args:
            agent_id: Agent ID
            name: 服务名称

        Returns:
            会话对象或None

        Note:
            会话数据总是存储在内存中，不会持久化到py-key-value存储，
            因为MCP Session对象不可序列化。
            这是同步方法且保持同步。
        """
        session = self.sessions.get(agent_id, {}).get(name)
        self._logger.debug(f"[SESSION_MANAGER] [GET] Got session: agent={agent_id}, service={name}, found={session is not None}")
        return session

    def set_session(self, agent_id: str, service_name: str, session: Any) -> None:
        """
        设置指定agent_id下服务的会话对象（同步，仅内存）

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            session: 要存储的会话对象

        Note:
            会话数据总是存储在内存中，不会持久化到py-key-value存储，
            因为MCP Session对象不可序列化。
            此方法包含防御性检查以防止意外的序列化。

        Raises:
            SessionSerializationError: 如果会话包含不可序列化的引用
        """
        # 导入异常映射器进行验证
        from ..exception_mapper import validate_session_serializable

        # 防御性检查：验证会话不包含不可序列化的引用
        validate_session_serializable(session, agent_id, service_name)

        # 存储到内存
        if agent_id not in self.sessions:
            self.sessions[agent_id] = {}
        self.sessions[agent_id][service_name] = session

        self._logger.debug(f"[SESSION_MANAGER] [SET] Set session: agent={agent_id}, service={service_name}")

    def get_session_for_tool(self, agent_id: str, tool_name: str) -> Optional[Any]:
        """
        获取指定agent_id下工具对应的服务会话

        Args:
            agent_id: Agent ID
            tool_name: 工具名称

        Returns:
            工具对应的会话对象或None
        """
        session = self.tool_to_session_map.get(agent_id, {}).get(tool_name)
        self._logger.debug(f"[SESSION_MANAGER] [GET] Got tool session: agent={agent_id}, tool={tool_name}, found={session is not None}")
        return session

    def clear_session(self, agent_id: str, service_name: str):
        """
        清除特定服务的会话

        Args:
            agent_id: Agent ID
            service_name: 服务名称
        """
        removed_session = None

        # 从服务会话中移除
        if agent_id in self.sessions and service_name in self.sessions[agent_id]:
            removed_session = self.sessions[agent_id].pop(service_name)

        # 从工具映射中移除相关的工具会话
        if agent_id in self.tool_to_session_map:
            tools_to_remove = []
            for tool_name, tool_session in self.tool_to_session_map[agent_id].items():
                if tool_session is removed_session:
                    tools_to_remove.append(tool_name)

            for tool_name in tools_to_remove:
                del self.tool_to_session_map[agent_id][tool_name]

        self._logger.debug(f"[SESSION_MANAGER] [CLEAR] Cleared session: agent={agent_id}, service={service_name}, removed={removed_session is not None}")

    def clear_all_sessions(self, agent_id: str):
        """
        清除指定agent_id的所有会话

        Args:
            agent_id: Agent ID
        """
        session_count = len(self.sessions.get(agent_id, {}))
        tool_mapping_count = len(self.tool_to_session_map.get(agent_id, {}))

        # 清除服务会话
        self.sessions.pop(agent_id, None)

        # 清除工具映射
        self.tool_to_session_map.pop(agent_id, None)

        self._logger.info(f"[SESSION_MANAGER] [CLEAR] Cleared all sessions: agent={agent_id}, services={session_count}, tools={tool_mapping_count}")

    def add_tool_session_mapping(self, agent_id: str, tool_name: str, session: Any) -> None:
        """
        添加工具到会话的映射关系

        Args:
            agent_id: Agent ID
            tool_name: 工具名称
            session: 会话对象
        """
        if agent_id not in self.tool_to_session_map:
            self.tool_to_session_map[agent_id] = {}
        self.tool_to_session_map[agent_id][tool_name] = session

        self._logger.debug(f"[SESSION_MANAGER] [ADD] Added tool session mapping: agent={agent_id}, tool={tool_name}")

    def remove_tool_session_mapping(self, agent_id: str, tool_name: str) -> Optional[Any]:
        """
        移除工具到会话的映射关系

        Args:
            agent_id: Agent ID
            tool_name: 工具名称

        Returns:
            被移除的会话对象
        """
        removed_session = None

        if agent_id in self.tool_to_session_map and tool_name in self.tool_to_session_map[agent_id]:
            removed_session = self.tool_to_session_map[agent_id].pop(tool_name)

        self._logger.debug(f"[SESSION_MANAGER] [REMOVE] Removed tool session mapping: agent={agent_id}, tool={tool_name}, removed={removed_session is not None}")
        return removed_session

    def get_all_service_names(self, agent_id: str) -> List[str]:
        """
        获取指定agent_id下所有有会话的服务名称

        Args:
            agent_id: Agent ID

        Returns:
            服务名称列表
        """
        return list(self.sessions.get(agent_id, {}).keys())

    def get_all_tool_names(self, agent_id: str) -> List[str]:
        """
        获取指定agent_id下所有有会话映射的工具名称

        Args:
            agent_id: Agent ID

        Returns:
            工具名称列表
        """
        return list(self.tool_to_session_map.get(agent_id, {}).keys())

    def has_session(self, agent_id: str, service_name: str) -> bool:
        """
        检查指定agent_id下服务是否有会话

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            是否存在会话
        """
        return service_name in self.sessions.get(agent_id, {})

    def has_tool_session_mapping(self, agent_id: str, tool_name: str) -> bool:
        """
        检查指定agent_id下工具是否有会话映射

        Args:
            agent_id: Agent ID
            tool_name: 工具名称

        Returns:
            是否存在会话映射
        """
        return tool_name in self.tool_to_session_map.get(agent_id, {})

    def get_session_count(self, agent_id: str) -> int:
        """
        获取指定agent_id的会话数量

        Args:
            agent_id: Agent ID

        Returns:
            会话数量
        """
        return len(self.sessions.get(agent_id, {}))

    def get_tool_mapping_count(self, agent_id: str) -> int:
        """
        获取指定agent_id的工具映射数量

        Args:
            agent_id: Agent ID

        Returns:
            工具映射数量
        """
        return len(self.tool_to_session_map.get(agent_id, {}))

    def get_agent_ids_with_sessions(self) -> List[str]:
        """
        获取所有有会话的agent_id列表

        Returns:
            agent_id列表
        """
        return list(self.sessions.keys())

    def get_agent_ids_with_tool_mappings(self) -> List[str]:
        """
        获取所有有工具映射的agent_id列表

        Returns:
            agent_id列表
        """
        return list(self.tool_to_session_map.keys())

    def get_stats(self) -> Dict[str, Any]:
        """
        获取会话管理器的统计信息

        Returns:
            统计信息字典
        """
        total_sessions = sum(len(services) for services in self.sessions.values())
        total_tool_mappings = sum(len(tools) for tools in self.tool_to_session_map.values())

        return {
            "total_sessions": total_sessions,
            "total_tool_mappings": total_tool_mappings,
            "agents_with_sessions": len(self.sessions),
            "agents_with_tool_mappings": len(self.tool_to_session_map),
            "namespace": self._namespace
        }
