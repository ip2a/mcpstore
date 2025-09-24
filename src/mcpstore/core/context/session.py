"""
MCPStore Session Module
User-friendly Session class that wraps AgentSession with rich functionality
"""

import logging
import asyncio
from datetime import datetime
from typing import Dict, List, Optional, Any, Union, TYPE_CHECKING

from mcpstore.core.models.tool import ToolInfo
from .types import ContextType

if TYPE_CHECKING:
    from mcpstore.core.agents.session_manager import AgentSession
    from .base_context import MCPStoreContext

logger = logging.getLogger(__name__)


class Session:
    """
    User-friendly Session class
    
    This class provides a clean, object-oriented interface for session management,
    wrapping the existing AgentSession with user-friendly methods that follow
    the two-word naming convention.
    
    Design principles:
    - Encapsulates existing AgentSession without replacing it
    - Provides chainable methods for fluent API
    - Follows two-word naming convention (bind_service, use_tool, etc.)
    - Reuses existing service discovery and connection logic
    """
    
    def __init__(self, context: 'MCPStoreContext', session_id: str, agent_session: 'AgentSession'):
        """
        Initialize Session object
        
        Args:
            context: MCPStoreContext instance for service operations
            session_id: User-friendly session identifier
            agent_session: Underlying AgentSession object
        """
        self._context = context
        self._session_id = session_id
        self._agent_session = agent_session
        self._is_active = True
        
        logger.info(f"[SESSION:{session_id}] Initialized session for agent {agent_session.agent_id}")
    
    # === Core Properties ===
    
    @property
    def session_id(self) -> str:
        """Get user-friendly session ID"""
        return self._session_id
    
    @property
    def is_active(self) -> bool:
        """Check if session is active"""
        return self._is_active and self._agent_session is not None
    
    @property
    def service_count(self) -> int:
        """Get number of bound services"""
        return len(self._agent_session.services) if self._agent_session else 0
    
    @property
    def tool_count(self) -> int:
        """Get number of available tools"""
        return len(self._agent_session.tools) if self._agent_session else 0
    
    # === Service Management ===
    
    def bind_service(self, service_name: str) -> 'Session':
        """
        Bind service to session
        
        This method creates a FastMCP Client for the service and caches it
        in the session for reuse. The Client connection will be maintained
        until the session is closed.
        
        Args:
            service_name: Name of the service to bind
            
        Returns:
            Session: Self for method chaining
            
        Example:
            session.bind_service("browser")
            session.bind_service("weather")
        """
        if not self.is_active:
            raise RuntimeError(f"Session {self._session_id} is not active")
            
        try:
            # Check if service is already bound
            if service_name in self._agent_session.services:
                logger.info(f"[SESSION:{self._session_id}] Service '{service_name}' already bound")
                return self
            
            # Use context's sync helper to run async service binding
            # Bind service quickly; no need for background loop and long timeout
            self._context._sync_helper.run_async(
                self._bind_service_async(service_name),
                timeout=20.0,
                force_background=True
            )
            
            logger.info(f"[SESSION:{self._session_id}] Successfully bound service '{service_name}'")
            return self
            
        except Exception as e:
            logger.error(f"[SESSION:{self._session_id}] Failed to bind service '{service_name}': {e}")
            raise
    
    async def _bind_service_async(self, service_name: str):
        """
        Internal async method to bind service

        This method marks the service as bound to the session and eagerly creates
        a persistent FastMCP client to reduce latency on the first tool call.
        """
        # Mark service as bound (placeholder client)
        self._agent_session.add_service(service_name, None)

        # Eagerly create and cache persistent client to avoid first-call delay
        try:
            orchestrator = self._context._store.orchestrator
            client = await orchestrator._create_persistent_client(self._agent_session, service_name)
            if client:
                logger.info(f"[SESSION:{self._session_id}] Eager persistent client created for service '{service_name}'")
        except Exception as e:
            # Fallback: orchestrator will lazily create on first use
            logger.warning(f"[SESSION:{self._session_id}] Eager client creation failed for '{service_name}', will create lazily: {e}")

        # Update session activity
        self._agent_session.update_activity()

        logger.info(f"[SESSION:{self._session_id}] Service '{service_name}' marked as bound")
        logger.debug(f"[SESSION:{self._session_id}] Service '{service_name}' bound to session")
    
    # === Tool Execution ===
    
    def use_tool(self, tool_name: str, arguments: Dict[str, Any] = None, **kwargs) -> Any:
        """
        Use tool within this session

        This method executes tools using the cached FastMCP Client connections,
        ensuring that stateful services (like browser) maintain their state
        across multiple tool calls.

        Args:
            tool_name: Name of the tool to execute
            arguments: Tool arguments
            **kwargs: Additional execution options

        Returns:
            Any: Tool execution result

        Example:
            result = session.use_tool("browser_navigate", {"url": "https://baidu.com"})
            result = session.use_tool("browser_click", {"selector": "#search"})
        """
        if not self.is_active:
            raise RuntimeError(f"Session {self._session_id} is not active")

        # 🎯 TIMING: Add precise timing to locate 30s delay
        import time
        t_start = time.perf_counter()
        logger.debug(f"[TIMING] Session.use_tool START: {tool_name}")

        # Use context's sync helper for async execution
        # 🎯 FIX: Remove force_background=True to avoid cross-thread race conditions
        # Use the same simple waiting mechanism as the regular LangChain adapter
        # Allow long startup for local stdio services (e.g., first npx run)
        wrapper_timeout = kwargs.get('timeout', 180.0)

        t_before_run_async = time.perf_counter()
        logger.debug(f"[TIMING] Before run_async: +{(t_before_run_async - t_start)*1000:.1f}ms")

        result = self._context._sync_helper.run_async(
            self.use_tool_async(tool_name, arguments, **kwargs),
            timeout=wrapper_timeout,
            force_background=True
        )

        t_after_run_async = time.perf_counter()
        logger.debug(f"[TIMING] After run_async: +{(t_after_run_async - t_before_run_async)*1000:.1f}ms, total: +{(t_after_run_async - t_start)*1000:.1f}ms")

        return result
    
    async def use_tool_async(self, tool_name: str, arguments: Dict[str, Any] = None, **kwargs) -> Any:
        """
        Use tool within this session (async version)
        
        This method routes tool execution through the session-aware execution path,
        which will reuse cached FastMCP Client connections.
        """
        arguments = arguments or {}
        
        logger.info(f"[SESSION:{self._session_id}] Executing tool '{tool_name}' with args: {arguments}")
        
        # Fast path: avoid pre-fetching available tools to determine service.
        # Tool name resolution and service binding will be handled downstream by call_tool_async
        # and orchestrator's session-aware execution path.
        result = await self._context.call_tool_async(
            tool_name=tool_name, 
            args=arguments, 
            session_id=self._session_id,
            **kwargs
        )
        
        # Update session activity
        self._agent_session.update_activity()
        
        logger.info(f"[SESSION:{self._session_id}] Tool '{tool_name}' executed successfully")
        return result
    
    # === Session Information ===
    
    def session_info(self) -> Dict[str, Any]:
        """
        Get comprehensive session information
        
        Returns:
            Dict containing session status, statistics, and metadata
        """
        if not self._agent_session:
            return {
                "session_id": self._session_id,
                "is_active": False,
                "error": "Session not initialized"
            }
        
        return {
            "session_id": self._session_id,
            "agent_id": self._agent_session.agent_id,
            "is_active": self.is_active,
            "service_count": self.service_count,
            "tool_count": self.tool_count,
            "created_at": self._agent_session.created_at.isoformat(),
            "last_active": self._agent_session.last_active.isoformat(),
            "bound_services": list(self._agent_session.services.keys()),
            "available_tools": list(self._agent_session.tools.keys())
        }
    
    def list_services(self) -> List[str]:
        """
        List all services bound to this session
        
        Returns:
            List of service names
        """
        return list(self._agent_session.services.keys()) if self._agent_session else []
    
    def list_tools(self) -> List[str]:
        """
        List all tools available in this session
        
        Returns:
            List of tool names
        """
        return list(self._agent_session.tools.keys()) if self._agent_session else []
    
    def connection_status(self) -> Dict[str, Any]:
        """
        Get connection status for all bound services
        
        Returns:
            Dict with service connection status information
        """
        if not self._agent_session:
            return {}
        
        status = {}
        for service_name, client in self._agent_session.services.items():
            # Check client connection status
            is_connected = hasattr(client, 'is_connected') and getattr(client, 'is_connected', False)
            status[service_name] = {
                "connected": is_connected,
                "client_type": type(client).__name__
            }
        
        return status
    
    # === Session Lifecycle Management ===
    
    def extend_session(self, additional_seconds: int = 3600) -> 'Session':
        """
        Extend session timeout
        
        Args:
            additional_seconds: Additional time to extend session (default: 1 hour)
            
        Returns:
            Session: Self for method chaining
        """
        if self._agent_session:
            # Update last_active to effectively extend the session
            self._agent_session.last_active = datetime.now()
            logger.info(f"[SESSION:{self._session_id}] Session extended by {additional_seconds} seconds")
        
        return self
    
    def clear_cache(self) -> 'Session':
        """
        Clear session cache (tools cache, not service connections)
        
        This clears the tools cache but keeps service connections alive.
        Use this if you want to refresh tool discovery without reconnecting services.
        
        Returns:
            Session: Self for method chaining
        """
        if self._agent_session:
            self._agent_session.tools.clear()
            logger.info(f"[SESSION:{self._session_id}] Session cache cleared")
        
        return self
    
    def restart_session(self) -> 'Session':
        """
        Restart session (reconnect all services)
        
        This closes all current connections and re-establishes them.
        Use this if you encounter connection issues.
        
        Returns:
            Session: Self for method chaining
        """
        if not self._agent_session:
            return self
        
        try:
            # Store service names before closing connections
            service_names = list(self._agent_session.services.keys())
            
            # Close all existing connections
            for service_name, client in self._agent_session.services.items():
                try:
                    # Close client connection if it has close method
                    # Best-effort async close without blocking current thread
                    try:
                        import asyncio as _asyncio
                        loop = self._context._sync_helper._ensure_loop()
                        if hasattr(client, 'close'):
                            _asyncio.run_coroutine_threadsafe(client.close(), loop)
                        elif hasattr(client, '_disconnect'):
                            _asyncio.run_coroutine_threadsafe(client._disconnect(), loop)
                        elif hasattr(client, '__aexit__'):
                            _asyncio.run_coroutine_threadsafe(client.__aexit__(None, None, None), loop)
                    except Exception as _e:
                        logger.warning(f"[SESSION:{self._session_id}] Error scheduling client close for {service_name}: {_e}")
                except Exception as e:
                    logger.warning(f"[SESSION:{self._session_id}] Error closing client for {service_name}: {e}")
            
            # Clear services and tools
            self._agent_session.services.clear()
            self._agent_session.tools.clear()
            
            # Reconnect all services
            for service_name in service_names:
                self.bind_service(service_name)
            
            logger.info(f"[SESSION:{self._session_id}] Session restarted successfully")
            
        except Exception as e:
            logger.error(f"[SESSION:{self._session_id}] Error restarting session: {e}")
            raise
        
        return self
    
    def close_session(self) -> None:
        """
        Close session and cleanup all resources
        
        This method closes all FastMCP Client connections and marks the session
        as inactive. After calling this method, the session cannot be used.
        """
        if not self.is_active:
            logger.warning(f"[SESSION:{self._session_id}] Session already closed")
            return
        
        try:
            # Close all client connections
            if self._agent_session:
                for service_name, client in self._agent_session.services.items():
                    try:
                        # Best-effort async close without blocking current thread
                        try:
                            import asyncio as _asyncio
                            loop = self._context._sync_helper._ensure_loop()
                            if hasattr(client, 'close'):
                                _asyncio.run_coroutine_threadsafe(client.close(), loop)
                            elif hasattr(client, '_disconnect'):
                                _asyncio.run_coroutine_threadsafe(client._disconnect(), loop)
                            elif hasattr(client, '__aexit__'):
                                _asyncio.run_coroutine_threadsafe(client.__aexit__(None, None, None), loop)
                        except Exception as _e:
                            logger.warning(f"[SESSION:{self._session_id}] Error scheduling client close for {service_name}: {_e}")
                    except Exception as e:
                        logger.warning(f"[SESSION:{self._session_id}] Error closing client for {service_name}: {e}")
                
                # Clear all caches
                self._agent_session.services.clear()
                self._agent_session.tools.clear()
            
            # Mark session as inactive
            self._is_active = False
            
            logger.info(f"[SESSION:{self._session_id}] Session closed successfully")
            
        except Exception as e:
            logger.error(f"[SESSION:{self._session_id}] Error closing session: {e}")
            self._is_active = False  # Mark as inactive even if cleanup failed
            raise
    
    # === Magic Methods ===
    
    def __str__(self) -> str:
        """String representation of session"""
        return f"Session(id={self._session_id}, services={self.service_count}, tools={self.tool_count}, active={self.is_active})"
    
    def __repr__(self) -> str:
        """Detailed representation of session"""
        return f"Session(session_id='{self._session_id}', agent_id='{self._agent_session.agent_id if self._agent_session else None}', active={self.is_active})"
    
    def __enter__(self):
        """Context manager entry (for synchronous use)"""
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit (for synchronous use)"""
        self.close_session()


class SessionContext:
    """
    Asynchronous context manager for session lifecycle management
    
    This class provides automatic session creation and cleanup using
    Python's async context manager protocol.
    
    Example:
        async with store.for_store().with_session("browser_task") as session:
            session.bind_service("browser")
            result = await session.use_tool_async("browser_navigate", {"url": "https://baidu.com"})
        # Session automatically closed
    """
    
    def __init__(self, context: 'MCPStoreContext', session_id: str):
        """
        Initialize session context manager

        Args:
            context: MCPStoreContext instance
            session_id: User-friendly session identifier
        """
        self._context = context
        self._session_id = session_id
        self._session: Optional[Session] = None
        # Track previous active session to support nested contexts
        self._prev_active_session: Optional[Session] = None

        logger.debug(f"[SESSION_CONTEXT:{session_id}] Context manager initialized")

    async def __aenter__(self) -> Session:
        """
        Async context manager entry

        Creates and returns a new session and sets it as the active session
        for implicit routing within the context scope.

        Returns:
            Session: New session instance
        """
        try:
            # Create session using context's session management
            self._session = await self._create_session_async()
            # Save previous and set current as active for implicit routing
            self._prev_active_session = getattr(self._context, "_active_session", None)
            self._context._active_session = self._session
            logger.info(f"[SESSION_CONTEXT:{self._session_id}] Session created successfully; set as active")
            return self._session

        except Exception as e:
            logger.error(f"[SESSION_CONTEXT:{self._session_id}] Failed to create session: {e}")
            raise

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """
        Async context manager exit

        Restore previous active session and close the current session.

        Args:
            exc_type: Exception type (if any)
            exc_val: Exception value (if any)
            exc_tb: Exception traceback (if any)
        """
        # Restore previous active session if we are the current active
        try:
            if getattr(self._context, "_active_session", None) is self._session:
                self._context._active_session = self._prev_active_session
        except Exception:
            pass

        if self._session:
            try:
                # Close session asynchronously
                await self._close_session_async()
                logger.info(f"[SESSION_CONTEXT:{self._session_id}] Session closed successfully")

            except Exception as e:
                logger.error(f"[SESSION_CONTEXT:{self._session_id}] Error closing session: {e}")
                # Don't raise the exception to avoid masking the original exception

        # Clear reference
        self._session = None

    async def _create_session_async(self) -> Session:
        """
        Internal method to create session asynchronously
        
        Now that SessionManagementMixin is integrated, we can use it to create sessions.
        """
        # Use the context's session management to create a session
        return self._context.create_session(self._session_id)
    
    async def _close_session_async(self):
        """
        Internal method to close session asynchronously
        """
        if self._session:
            # Use the session's close method but run it in async context
            # Since close_session is synchronous, we don't need additional async handling
            self._session.close_session()
    
    # === Synchronous Context Manager Protocol ===
    
    def __enter__(self) -> Session:
        """
        Synchronous context manager entry

        Creates and returns a new session using sync helper, and sets it as
        the active session for implicit routing within the scope.

        Returns:
            Session: New session instance
        """
        try:
            # Use sync helper to run async session creation
            from mcpstore.core.utils.async_sync_helper import get_global_helper
            sync_helper = get_global_helper()
            self._session = sync_helper.run_async(
                self._create_session_async(),
                force_background=True
            )
            # Save previous and set current as active for implicit routing
            self._prev_active_session = getattr(self._context, "_active_session", None)
            self._context._active_session = self._session
            logger.info(f"[SESSION_CONTEXT:{self._session_id}] Session created successfully (sync); set as active")
            return self._session

        except Exception as e:
            logger.error(f"[SESSION_CONTEXT:{self._session_id}] Failed to create session (sync): {e}")
            raise

    def __exit__(self, exc_type, exc_val, exc_tb):
        """
        Synchronous context manager exit

        Restore previous active session and close the current session.

        Args:
            exc_type: Exception type (if any)
            exc_val: Exception value (if any)
            exc_tb: Exception traceback (if any)
        """
        try:
            # Restore previous active session if we are the current active
            try:
                if getattr(self._context, "_active_session", None) is self._session:
                    self._context._active_session = self._prev_active_session
            except Exception:
                pass

            if self._session:
                # Close session synchronously to avoid background run_async timeouts
                try:
                    self._session.close_session()
                    logger.info(f"[SESSION_CONTEXT:{self._session_id}] Session closed successfully (sync)")
                except Exception as _e:
                    logger.error(f"[SESSION_CONTEXT:{self._session_id}] Error during session close (sync): {_e}")
            else:
                logger.warning(f"[SESSION_CONTEXT:{self._session_id}] No session to close (sync)")

        except Exception as e:
            logger.error(f"[SESSION_CONTEXT:{self._session_id}] Error closing session (sync): {e}")
            # Don't re-raise exceptions in __exit__ unless critical

        return False  # Don't suppress exceptions from the with block
