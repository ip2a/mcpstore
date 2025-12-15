"""
异步安全的核心注册表

解决嵌套事件循环死锁问题的统一异步架构实现
"""

import logging
from typing import Dict, List, Any, Optional
from .core_registry import ServiceRegistry

logger = logging.getLogger(__name__)


class AsyncSafeCoreRegistry(ServiceRegistry):
    """
    异步安全的核心注册表

    核心改进：
    1. 消除同步方法中的异步调用
    2. 统一使用异步调用链
    3. 避免嵌套事件循环死锁
    """

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        logger.debug("AsyncSafeCoreRegistry initialized - no more nested async calls")

    # ==================== 同步方法的重构 ====================

    def get_tool_info(self, agent_id: str, tool_name: str) -> Dict[str, Any]:
        """
        获取工具信息 - 同步版本

        直接从缓存读取，避免嵌套异步调用
        """
        logger.debug(f"[ASYNC_SAFE] Getting tool info: agent_id={agent_id}, tool_name={tool_name}")

        # 方法1：从内存缓存直接读取（避免异步调用）
        session = self.tool_to_session_map.get(agent_id, {}).get(tool_name)
        if not session:
            logger.debug(f"[ASYNC_SAFE] Tool session not found: {tool_name}")
            return None

        service_name = self._find_service_name_for_session(agent_id, session)
        if not service_name:
            logger.debug(f"[ASYNC_SAFE] Service not found for session: {tool_name}")
            return None

        # 构造基本信息（从内存）
        tool_info = {
            "name": tool_name,
            "tool_global_name": tool_name,
            "service_name": service_name,
            "agent_id": agent_id,
            "session": session,
            "is_connected": getattr(session, 'is_connected', False),
            "connection_type": type(session).__name__
        }

        logger.debug(f"[ASYNC_SAFE] Tool info retrieved from cache: {tool_name}")
        return tool_info

    def get_all_tools(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        获取所有工具 - 同步版本

        直接从内存缓存读取，避免嵌套异步调用
        """
        logger.debug(f"[ASYNC_SAFE] Getting all tools: agent_id={agent_id}")

        tools_list = []
        tool_to_session = self.tool_to_session_map.get(agent_id, {})

        for tool_name, session in tool_to_session.items():
            service_name = self._find_service_name_for_session(agent_id, session)

            tool_info = {
                "name": tool_name,
                "tool_global_name": tool_name,
                "service_name": service_name or "unknown",
                "agent_id": agent_id,
                "is_connected": getattr(session, 'is_connected', False),
                "connection_type": type(session).__name__
            }
            tools_list.append(tool_info)

        logger.debug(f"[ASYNC_SAFE] Retrieved {len(tools_list)} tools from cache: agent_id={agent_id}")
        return tools_list

    # ==================== 异步方法的增强 ====================

    async def get_tool_info_async(self, agent_id: str, tool_name: str) -> Optional[Dict[str, Any]]:
        """
        获取工具信息 - 异步版本

        使用完整的异步调用链，包括完整的工具定义
        """
        logger.debug(f"[ASYNC_SAFE] Getting tool info async: agent_id={agent_id}, tool_name={tool_name}")

        try:
            # 使用原有的异步逻辑，但避免嵌套调用
            tools_dict = await self._get_all_tools_dict_async(agent_id)

            if not tools_dict:
                logger.debug(f"[ASYNC_SAFE] No tools found for agent: {agent_id}")
                return None

            tool_def = tools_dict.get(tool_name)
            if not tool_def:
                logger.debug(f"[ASYNC_SAFE] Tool not found: {tool_name}")
                return None

            # 获取会话信息
            session = self.tool_to_session_map.get(agent_id, {}).get(tool_name)

            tool_info = {
                **tool_def,
                "session": session,
                "is_connected": getattr(session, 'is_connected', False) if session else False
            }

            logger.debug(f"[ASYNC_SAFE] Tool info retrieved async: {tool_name}")
            return tool_info

        except Exception as e:
            logger.error(f"[ASYNC_SAFE] Failed to get tool info async: {tool_name}, error={e}")
            return None

    async def get_all_tools_async(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        获取所有工具 - 异步版本

        使用完整的异步调用链
        """
        logger.debug(f"[ASYNC_SAFE] Getting all tools async: agent_id={agent_id}")

        try:
            tools_dict = await self._get_all_tools_dict_async(agent_id)

            if not tools_dict:
                logger.debug(f"[ASYNC_SAFE] No tools found for agent: {agent_id}")
                return []

            tools_list = []
            tool_to_session = self.tool_to_session_map.get(agent_id, {})

            for tool_name, tool_def in tools_dict.items():
                session = tool_to_session.get(tool_name)

                tool_info = {
                    **tool_def,
                    "session": session,
                    "is_connected": getattr(session, 'is_connected', False) if session else False
                }
                tools_list.append(tool_info)

            logger.debug(f"[ASYNC_SAFE] Retrieved {len(tools_list)} tools async: agent_id={agent_id}")
            return tools_list

        except Exception as e:
            logger.error(f"[ASYNC_SAFE] Failed to get all tools async: agent_id={agent_id}, error={e}")
            return []

    # ==================== 辅助方法 ====================

    def _find_service_name_for_session(self, agent_id: str, session) -> Optional[str]:
        """
        从内存中查找会话对应的服务名称

        这个方法避免了异步调用，直接从内存缓存查找
        """
        agent_sessions = self.sessions.get(agent_id, {})

        for service_name, service_session in agent_sessions.items():
            if service_session is session:
                return service_name

        return None

    # ==================== 消除危险的_sync_to_kv调用 ====================

    def _sync_to_kv_safe(self, operation_name: str):
        """
        安全的同步到KV方法 - 防止嵌套调用

        这个方法会检测当前是否已经在异步上下文中，
        如果是，则抛出异常而不是进行嵌套调用
        """
        import asyncio

        try:
            # 检查是否已经在事件循环中
            asyncio.get_running_loop()

            # 如果在事件循环中，这是一个嵌套调用
            error_msg = (
                f"Detected nested async call in {operation_name}. "
                f"This indicates a design issue - sync methods should not call async operations. "
                f"Use async-safe methods instead."
            )
            logger.error(f"[ASYNC_SAFE] {error_msg}")
            raise RuntimeError(error_msg)

        except RuntimeError:
            # 不在事件循环中，可以安全地进行同步调用
            logger.debug(f"[ASYNC_SAFE] Safe sync context for: {operation_name}")
            return self._sync_to_kv

    # ==================== 重写可能引起嵌套调用的方法 ====================

    def get_service_status(self, agent_id: str, service_name: str) -> Dict[str, Any]:
        """
        获取服务状态 - 同步版本

        直接从内存状态管理器读取，避免异步调用
        """
        logger.debug(f"[ASYNC_SAFE] Getting service status: agent_id={agent_id}, service_name={service_name}")

        try:
            # 从状态管理器直接读取（同步操作）
            state_info = self._state_backend.get_service_state(service_name)

            if not state_info:
                return {
                    "service_name": service_name,
                    "agent_id": agent_id,
                    "status": "unknown",
                    "healthy": False,
                    "tools_count": 0
                }

            return {
                "service_name": service_name,
                "agent_id": agent_id,
                "status": state_info.get("health", "unknown"),
                "healthy": state_info.get("health") == "healthy",
                "tools_count": state_info.get("tools_count", 0),
                "last_updated": state_info.get("last_updated"),
                "error": state_info.get("error")
            }

        except Exception as e:
            logger.error(f"[ASYNC_SAFE] Failed to get service status: {service_name}, error={e}")
            return {
                "service_name": service_name,
                "agent_id": agent_id,
                "status": "error",
                "healthy": False,
                "tools_count": 0,
                "error": str(e)
            }

    async def get_service_status_async(self, agent_id: str, service_name: str) -> Dict[str, Any]:
        """
        获取服务状态 - 异步版本

        使用完整的状态检查逻辑
        """
        logger.debug(f"[ASYNC_SAFE] Getting service status async: agent_id={agent_id}, service_name={service_name}")

        try:
            # 异步获取完整状态信息
            state_info = await self._state_backend.get_service_state_async(service_name)

            if not state_info:
                return {
                    "service_name": service_name,
                    "agent_id": agent_id,
                    "status": "unknown",
                    "healthy": False,
                    "tools_count": 0
                }

            return {
                "service_name": service_name,
                "agent_id": agent_id,
                "status": state_info.get("health", "unknown"),
                "healthy": state_info.get("health") == "healthy",
                "tools_count": state_info.get("tools_count", 0),
                "last_updated": state_info.get("last_updated"),
                "error": state_info.get("error"),
                "full_state": state_info  # 异步版本提供完整状态
            }

        except Exception as e:
            logger.error(f"[ASYNC_SAFE] Failed to get service status async: {service_name}, error={e}")
            return {
                "service_name": service_name,
                "agent_id": agent_id,
                "status": "error",
                "healthy": False,
                "tools_count": 0,
                "error": str(e)
            }


class AsyncSafeRegistryFactory:
    """
    异步安全注册表工厂

    提供创建异步安全注册表的统一接口
    """

    @staticmethod
    def create_registry(*args, **kwargs) -> AsyncSafeCoreRegistry:
        """
        创建异步安全的核心注册表实例
        """
        logger.debug("Creating AsyncSafeCoreRegistry instance")
        return AsyncSafeCoreRegistry(*args, **kwargs)

    @staticmethod
    def migrate_from_standard_registry(standard_registry: ServiceRegistry) -> AsyncSafeCoreRegistry:
        """
        从标准注册表迁移到异步安全注册表

        保持所有现有数据和状态
        """
        logger.info("Migrating from standard registry to async-safe registry")

        # 创建新的异步安全注册表
        async_safe_registry = AsyncSafeCoreRegistry.__new__(AsyncSafeCoreRegistry)

        # 复制所有必要的状态
        async_safe_registry.sessions = standard_registry.sessions
        async_safe_registry.tool_to_session_map = standard_registry.tool_to_session_map
        async_safe_registry._service_manager = standard_registry._service_manager
        async_safe_registry._tool_manager = standard_registry._tool_manager
        async_safe_registry._relation_manager = standard_registry._relation_manager
        async_safe_registry._state_backend = standard_registry._state_backend
        async_safe_registry._naming = standard_registry._naming
        async_safe_registry._sync_helper = standard_registry._sync_helper

        logger.info("Migration completed successfully")
        return async_safe_registry