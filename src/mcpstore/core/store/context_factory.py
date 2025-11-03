"""
上下文工厂模块
负责处理 MCPStore 的上下文创建和管理功能
"""

import logging
from typing import Dict, List, Optional

from mcpstore.core.context import MCPStoreContext
from mcpstore.core.context.store_proxy import StoreProxy
from mcpstore.core.context.agent_proxy import AgentProxy

logger = logging.getLogger(__name__)


class ContextFactoryMixin:
    """上下文工厂 Mixin"""
    
    def _create_store_context(self) -> MCPStoreContext:
        """Create store-level context"""
        return MCPStoreContext(self)

    def get_store_context(self) -> MCPStoreContext:
        """Get store-level context"""
        return self._store_context

    def _create_agent_context(self, agent_id: str) -> MCPStoreContext:
        """Create agent-level context"""
        return MCPStoreContext(self, agent_id)

    def for_store(self) -> StoreProxy:
        """Get store-level object (proxy)"""
        return self._store_context.for_store()

    def for_agent(self, agent_id: str) -> AgentProxy:
        """Get agent-level object (proxy) with caching"""
        if agent_id not in self._context_cache:
            self._context_cache[agent_id] = self._create_agent_context(agent_id)
        return self._context_cache[agent_id].find_agent(agent_id)

    # -- Objectified helpers (non-breaking) --
    def for_store_proxy(self):
        """Alias of for_store() for backward compatibility."""
        return self.for_store()

    def for_agent_proxy(self, agent_id: str):
        """Alias of for_agent() for backward compatibility."""
        return self.for_agent(agent_id)

    # 委托方法 - 保持向后兼容性
    async def add_service(self, service_names: List[str] = None, agent_id: Optional[str] = None, **kwargs) -> bool:
        """
        委托给 Context 层的 add_service 方法
        保持向后兼容性

        Args:
            service_names: 服务名称列表（兼容旧版API）
            agent_id: Agent ID（可选）
            **kwargs: 其他参数传递给 Context 层

        Returns:
            bool: 操作是否成功
        """
        context = self.for_agent(agent_id) if agent_id else self.for_store()

        # 如果提供了 service_names，转换为新的格式
        if service_names:
            # 兼容旧版 API，将 service_names 转换为配置格式
            config = {"service_names": service_names}
            await context.add_service_async(config, **kwargs)
        else:
            # 新版 API，直接传递参数
            await context.add_service_async(**kwargs)

        return True

    def check_services(self, agent_id: Optional[str] = None) -> Dict[str, str]:
        """
        委托给 Context 层的 check_services 方法
        兼容旧版API
        """
        context = self.for_agent(agent_id) if agent_id else self.for_store()
        return context.check_services()
