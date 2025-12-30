"""
Context factory module
Responsible for handling MCPStore context creation and management functionality
"""

import logging
from typing import Dict, List, Optional

from mcpstore.core.context import MCPStoreContext
from mcpstore.core.context.agent_proxy import AgentProxy
from mcpstore.core.context.store_proxy import StoreProxy

logger = logging.getLogger(__name__)


class ContextFactoryMixin:
    """Context factory Mixin"""
    
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

    def find_cache(self):
        """Get global cache proxy (store scope)."""
        return self._store_context.find_cache()

    def for_agent(self, agent_id: str) -> AgentProxy:
        """
        Get agent-level object (proxy) with unified caching.

        Uses the centralized AgentProxy caching system to ensure that the same
        agent_id always returns the same AgentProxy instance across all access
        methods in the MCPStore.

        Args:
            agent_id: Unique identifier for the agent

        Returns:
            AgentProxy: Cached or newly created AgentProxy instance
        """
        # Create or reuse agent context (still cached for efficiency)
        if agent_id not in self._context_cache:
            self._context_cache[agent_id] = self._create_agent_context(agent_id)

        agent_context = self._context_cache[agent_id]

        # Use unified AgentProxy caching system
        return self._get_or_create_agent_proxy(agent_context, agent_id)

    
    # Delegation methods - maintain backward compatibility
    async def add_service(self, service_names: List[str] = None, agent_id: Optional[str] = None, **kwargs) -> bool:
        """
        Delegate to Context layer add_service method
        Maintain backward compatibility

        Args:
            service_names: List of service names (compatible with old API)
            agent_id: Agent ID (optional)
            **kwargs: Other parameters passed to Context layer

        Returns:
            bool: Whether operation succeeded
        """
        context = self.for_agent(agent_id) if agent_id else self.for_store()

        # If service_names is provided, convert to new format
        if service_names:
            # Compatible with old API, convert service_names to config format
            config = {"service_names": service_names}
            await context.add_service_async(config, **kwargs)
        else:
            # New API, pass parameters directly
            await context.add_service_async(**kwargs)

        return True

    def check_services(self, agent_id: Optional[str] = None) -> Dict[str, str]:
        """
        Delegate to Context layer check_services method
        Compatible with old API
        """
        context = self.for_agent(agent_id) if agent_id else self.for_store()
        return context.check_services()
