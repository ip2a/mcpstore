"""
异步安全的服务管理

修复service_management.py中的嵌套异步调用问题
"""

import logging
from typing import Dict, Any, Optional

from .service_management import ServiceManagement
from ..utils.deadlock_safe_async_helper import get_deadlock_safe_helper

logger = logging.getLogger(__name__)


class AsyncSafeServiceManagement(ServiceManagement):
    """
    异步安全的服务管理

    核心改进：
    1. 消除get_service_info中的强制后台调用
    2. 使用缓存优先策略减少异步调用
    3. 提供完整的异步调用链
    """

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        # 使用死锁安全的异步助手
        self._safe_sync_helper = get_deadlock_safe_helper()

        # 服务信息缓存（避免重复的异步调用）
        self._service_info_cache: Dict[str, Dict[str, Any]] = {}
        self._cache_timeout = 5.0  # 5秒缓存

        logger.debug("AsyncSafeServiceManagement initialized with deadlock-safe helper")

    def get_service_info(self, name: str, use_cache: bool = True) -> Dict[str, Any]:
        """
        获取服务信息 - 同步版本

        使用缓存优先策略，避免嵌套异步调用

        Args:
            name: 服务名称
            use_cache: 是否使用缓存（默认True）

        Returns:
            服务信息字典
        """
        logger.debug(f"[ASYNC_SAFE_SM] Getting service info: {name}, use_cache={use_cache}")

        # 检查缓存
        if use_cache:
            cached_info = self._get_from_cache(name)
            if cached_info is not None:
                logger.debug(f"[ASYNC_SAFE_SM] Service info from cache: {name}")
                return cached_info

        try:
            # 方法1：从内存状态直接构造信息（避免异步调用）
            service_info = self._get_service_info_from_memory(name)

            if service_info is not None:
                # 更新缓存
                self._update_cache(name, service_info)
                logger.debug(f"[ASYNC_SAFE_SM] Service info from memory: {name}")
                return service_info

            # 方法2：使用异步安全助手进行调用
            if self._registry and hasattr(self._registry, 'get_tool_info'):
                # 使用注册表的同步方法
                service_info = self._get_service_info_from_registry(name)

                if service_info is not None:
                    self._update_cache(name, service_info)
                    logger.debug(f"[ASYNC_SAFE_SM] Service info from registry: {name}")
                    return service_info

            # 方法3：最后才考虑异步调用，但使用死锁安全机制
            logger.debug(f"[ASYNC_SAFE_SM] Falling back to async call for: {name}")
            return self._get_service_info_async_safe(name)

        except Exception as e:
            logger.error(f"[ASYNC_SAFE_SM] Failed to get service info: {name}, error={e}")
            # 返回基本错误信息而不是抛出异常
            return {
                "name": name,
                "error": str(e),
                "status": "error",
                "is_connected": False,
                "tools": []
            }

    def _get_from_cache(self, name: str) -> Optional[Dict[str, Any]]:
        """从缓存获取服务信息"""
        import time

        cache_entry = self._service_info_cache.get(name)
        if cache_entry is None:
            return None

        cached_time, cached_info = cache_entry
        current_time = time.time()

        if current_time - cached_time > self._cache_timeout:
            # 缓存过期
            del self._service_info_cache[name]
            return None

        return cached_info

    def _update_cache(self, name: str, info: Dict[str, Any]):
        """更新服务信息缓存"""
        import time
        self._service_info_cache[name] = (time.time(), info)

    def _get_service_info_from_memory(self, name: str) -> Optional[Dict[str, Any]]:
        """从内存状态获取服务信息"""
        try:
            if not self._registry:
                return None

            # 检查服务是否存在于内存中
            agent_id = self._get_agent_id_for_service(name)
            if agent_id is None:
                return None

            # 获取工具信息（从内存）
            tools_info = []
            if hasattr(self._registry, 'tool_to_session_map'):
                tool_to_session = self._registry.tool_to_session_map.get(agent_id, {})

                for tool_name, session in tool_to_session.items():
                    # 简单检查工具是否属于该服务
                    service_name = self._find_service_for_tool(agent_id, tool_name, name)
                    if service_name == name:
                        tools_info.append({
                            "name": tool_name,
                            "display_name": tool_name,
                            "description": "Tool from memory cache",
                            "is_connected": getattr(session, 'is_connected', False)
                        })

            # 构造基本服务信息
            service_info = {
                "name": name,
                "agent_id": agent_id,
                "is_connected": len(tools_info) > 0,
                "tools": tools_info,
                "tools_count": len(tools_info),
                "status": "connected" if tools_info else "disconnected",
                "source": "memory_cache"
            }

            return service_info

        except Exception as e:
            logger.debug(f"[ASYNC_SAFE_SM] Failed to get service info from memory: {name}, error={e}")
            return None

    def _get_service_info_from_registry(self, name: str) -> Optional[Dict[str, Any]]:
        """从注册表获取服务信息（同步方法）"""
        try:
            if not self._registry:
                return None

            agent_id = self._get_agent_id_for_service(name)
            if agent_id is None:
                return None

            # 使用注册表的同步方法获取工具信息
            if hasattr(self._registry, 'get_all_tools'):
                tools_list = self._registry.get_all_tools(agent_id)

                # 过滤出属于当前服务的工具
                service_tools = []
                for tool_info in tools_list:
                    if tool_info.get('service_name') == name:
                        service_tools.append({
                            "name": tool_info.get('name'),
                            "display_name": tool_info.get('display_name', tool_info.get('name')),
                            "description": tool_info.get('description', 'No description available'),
                            "is_connected": tool_info.get('is_connected', False)
                        })

                return {
                    "name": name,
                    "agent_id": agent_id,
                    "is_connected": len(service_tools) > 0,
                    "tools": service_tools,
                    "tools_count": len(service_tools),
                    "status": "connected" if service_tools else "disconnected",
                    "source": "registry_sync"
                }

            return None

        except Exception as e:
            logger.debug(f"[ASYNC_SAFE_SM] Failed to get service info from registry: {name}, error={e}")
            return None

    def _get_service_info_async_safe(self, name: str) -> Dict[str, Any]:
        """使用死锁安全机制获取服务信息"""
        try:
            # 使用死锁安全的异步助手
            if hasattr(self, 'get_service_info_async'):
                return self._safe_sync_helper.run_async(
                    self.get_service_info_async(name),
                    timeout=10.0,
                    operation_name=f"get_service_info_async:{name}",
                    force_background=True
                )
            else:
                # 降级到基本错误信息
                return {
                    "name": name,
                    "error": "Async method not available",
                    "status": "error",
                    "is_connected": False,
                    "tools": []
                }

        except Exception as e:
            logger.error(f"[ASYNC_SAFE_SM] Async-safe call failed: {name}, error={e}")
            return {
                "name": name,
                "error": str(e),
                "status": "error",
                "is_connected": False,
                "tools": []
            }

    def _get_agent_id_for_service(self, name: str) -> Optional[str]:
        """获取服务对应的agent_id"""
        if self._context_type.value == "store":
            return "global_agent_store"
        elif self._client_manager:
            return self._client_manager.get_agent_id()
        return None

    def _find_service_for_tool(self, agent_id: str, tool_name: str, target_service: str) -> Optional[str]:
        """查找工具所属的服务"""
        try:
            if not hasattr(self._registry, 'sessions'):
                return None

            agent_sessions = self._registry.sessions.get(agent_id, {})

            for service_name, session in agent_sessions.items():
                if service_name == target_service:
                    # 检查该会话是否有这个工具
                    if hasattr(self._registry, 'tool_to_session_map'):
                        tool_to_session = self._registry.tool_to_session_map.get(agent_id, {})
                        if tool_to_session.get(tool_name) is session:
                            return service_name

            return None

        except Exception as e:
            logger.debug(f"[ASYNC_SAFE_SM] Failed to find service for tool: {tool_name}, error={e}")
            return None

    def clear_cache(self, name: Optional[str] = None):
        """清除服务信息缓存"""
        if name:
            self._service_info_cache.pop(name, None)
            logger.debug(f"[ASYNC_SAFE_SM] Cache cleared for service: {name}")
        else:
            self._service_info_cache.clear()
            logger.debug("[ASYNC_SAFE_SM] All service info cache cleared")

    def get_cache_stats(self) -> Dict[str, Any]:
        """获取缓存统计信息"""
        import time
        current_time = time.time()

        cache_stats = {
            "total_cached": len(self._service_info_cache),
            "valid_entries": 0,
            "expired_entries": 0,
            "entries": []
        }

        for name, (cached_time, info) in self._service_info_cache.items():
            age = current_time - cached_time
            is_valid = age <= self._cache_timeout

            if is_valid:
                cache_stats["valid_entries"] += 1
            else:
                cache_stats["expired_entries"] += 1

            cache_stats["entries"].append({
                "name": name,
                "age": age,
                "is_valid": is_valid,
                "tools_count": info.get("tools_count", 0)
            })

        return cache_stats


class AsyncSafeServiceManagementFactory:
    """异步安全服务管理工厂"""

    @staticmethod
    def create_service_management(*args, **kwargs) -> AsyncSafeServiceManagement:
        """创建异步安全的服务管理实例"""
        logger.debug("Creating AsyncSafeServiceManagement instance")
        return AsyncSafeServiceManagement(*args, **kwargs)

    @staticmethod
    def migrate_from_standard_management(standard_management: ServiceManagement) -> AsyncSafeServiceManagement:
        """从标准服务管理迁移到异步安全版本"""
        logger.info("Migrating from standard service management to async-safe version")

        # 创建新的异步安全服务管理
        async_safe_management = AsyncSafeServiceManagement.__new__(AsyncSafeServiceManagement)

        # 复制所有必要的状态
        async_safe_management._context_type = standard_management._context_type
        async_safe_management._client_manager = standard_management._client_manager
        async_safe_management._store = standard_management._store
        async_safe_management._sync_helper = standard_management._sync_helper
        async_safe_management._registry = standard_management._registry

        # 初始化异步安全特有属性
        async_safe_management._safe_sync_helper = get_deadlock_safe_helper()
        async_safe_management._service_info_cache = {}
        async_safe_management._cache_timeout = 5.0

        logger.info("Service management migration completed successfully")
        return async_safe_management