"""
MCPStore Session Management Module
Session management functionality for MCPStoreContext
"""

import logging
from typing import Dict, List, Optional, Any, TYPE_CHECKING

from .types import ContextType

if TYPE_CHECKING:
    from .session import Session, SessionContext

logger = logging.getLogger(__name__)


class SessionManagementMixin:
    """
    Session management mixin for MCPStoreContext

    This mixin provides session management functionality that integrates
    with the existing SessionManager architecture. It follows the principle
    of maximum reuse and minimum modification.

    Key features:
    - Create, find, and manage Session objects
    - Support for both Store and Agent contexts
    - Automatic session mode (session_auto/session_manual)
    - Context manager support (with_session)
    - User-friendly session operations
    """

    def __init__(self):
        """
        Initialize session management state

        This will be called as part of MCPStoreContext.__init__()
        """
        # [AUTO] Auto session mode state
        self._auto_session_enabled = False
        self._auto_session: Optional['Session'] = None
        self._auto_session_config: Dict[str, Any] = {}

        # [CACHE] Session cache (avoid creating Session objects repeatedly)
        self._session_cache: Dict[str, 'Session'] = {}

        # [ACTIVE] Current active session (for implicit session routing)
        self._active_session: Optional['Session'] = None


        logger.debug(f"[SESSION_MANAGEMENT] Initialized for context type: {getattr(self, '_context_type', 'unknown')}")

    # === Core Session Operations ===

    def create_session(self, session_id: str, user_session_id: Optional[str] = None) -> 'Session':
        """
        Create a new session (Enhanced version)

        This method creates a new Session object that wraps an AgentSession,
        with optional cross-context access support through user_session_id.

        Args:
            session_id: User-friendly session identifier
            user_session_id: Optional global session ID for cross-context access

        Returns:
            Session: New session object

        Example:
            # Basic session
            session = store.for_store().create_session("browser_task")

            # Cross-context session
            session = store.for_store().create_session("browser_task", "global_browser_session")
            # Can be accessed from any context via user_session_id
        """
        try:
            # [AGENT] Get effective agent_id
            effective_agent_id = self._get_effective_agent_id()

            # [SESSION] Use enhanced SessionManager to create named session
            if hasattr(self._store.session_manager, 'create_named_session'):
                # Enhanced SessionManager - use named sessions
                agent_session = self._store.session_manager.create_named_session(
                    effective_agent_id, session_id, user_session_id
                )
            else:
                # Fallback to original SessionManager
                agent_session = self._store.session_manager.create_session(effective_agent_id)

            # [CREATE] User-friendly Session object
            from .session import Session
            session = Session(self, session_id, agent_session)

            # [CACHE] Session object
            cache_key = f"{effective_agent_id}:{session_id}"
            self._session_cache[cache_key] = session

            # [MAPPING] If user_session_id exists, also cache this mapping
            if user_session_id:
                self._session_cache[f"user:{user_session_id}"] = session

            logger.info(f"[SESSION_MANAGEMENT] Created session '{session_id}' for agent '{effective_agent_id}'" +
                       (f" with user session ID '{user_session_id}'" if user_session_id else ""))
            return session

        except Exception as e:
            logger.error(f"[SESSION_MANAGEMENT] Failed to create session '{session_id}': {e}")
            raise

    def find_session(self, session_id: Optional[str] = None, is_user_session_id: bool = False) -> Optional['Session']:
        """
        Find an existing session (Enhanced version)

        Args:
            session_id: Session identifier (optional)
                       If None, returns the auto session if enabled
            is_user_session_id: If True, treats session_id as a user_session_id for cross-context access

        Returns:
            Session object if found, None otherwise

        Example:
            # Local session access
            session = store.for_store().find_session("browser_task")

            # Cross-context access
            session = store.for_store().find_session("global_browser_session", is_user_session_id=True)

            # Auto session
            auto_session = store.for_store().find_session()
        """
        try:
            # [AUTO] If no session_id specified, return auto session
            if session_id is None:
                return self._auto_session if self._auto_session_enabled else None

            # [CROSS-CONTEXT] If cross-context access
            if is_user_session_id:
                # First check user session cache
                user_cache_key = f"user:{session_id}"
                if user_cache_key in self._session_cache:
                    session = self._session_cache[user_cache_key]
                    if session.is_active:
                        return session
                    else:
                        del self._session_cache[user_cache_key]

                # 使用增强的 SessionManager 查找
                if hasattr(self._store.session_manager, 'get_session_by_user_id'):
                    agent_session = self._store.session_manager.get_session_by_user_id(session_id)
                    if agent_session:
                        from .session import Session
                        session = Session(self, session_id, agent_session)
                        # 缓存用户会话映射
                        self._session_cache[user_cache_key] = session
                        return session

                return None

            # Regular local session lookup
            effective_agent_id = self._get_effective_agent_id()

            # Check cache
            cache_key = f"{effective_agent_id}:{session_id}"
            if cache_key in self._session_cache:
                session = self._session_cache[cache_key]
                # 验证底层 AgentSession 是否仍然有效
                if session.is_active:
                    return session
                else:
                    # 清理失效的缓存
                    del self._session_cache[cache_key]

            # Use enhanced SessionManager to find named session
            if hasattr(self._store.session_manager, 'get_named_session'):
                agent_session = self._store.session_manager.get_named_session(effective_agent_id, session_id)
            else:
                # Fallback to original SessionManager
                agent_session = self._store.session_manager.get_session(effective_agent_id)

            if agent_session:
                # 创建 Session 对象包装器
                from .session import Session
                session = Session(self, session_id, agent_session)
                # 更新缓存
                self._session_cache[cache_key] = session
                return session

            return None

        except Exception as e:
            logger.error(f"[SESSION_MANAGEMENT] Error finding session '{session_id}': {e}")
            return None

    def get_session(self, session_id: str) -> 'Session':
        """
        Get session (create if not exists)

        Args:
            session_id: Session identifier

        Returns:
            Session: Existing or new session object

        Example:
            session = store.for_store().get_session("browser_task")
        """
        session = self.find_session(session_id)
        if session:
            return session

        return self.create_session(session_id)

    def list_sessions(self) -> List['Session']:
        """
        List all sessions in current context

        Returns:
            List of Session objects

        Example:
            sessions = store.for_store().list_sessions()
            for session in sessions:
                print(f"Session: {session.session_id}")
        """
        try:
            sessions = []
            effective_agent_id = self._get_effective_agent_id()

            # Get AgentSession for current context
            agent_session = self._store.session_manager.get_session(effective_agent_id)
            if agent_session:
                # 为这个 AgentSession 创建一个默认的 Session 包装器
                from .session import Session
                default_session = Session(self, "default", agent_session)
                sessions.append(default_session)

            # Include auto session if available
            if self._auto_session_enabled and self._auto_session:
                if self._auto_session not in sessions:
                    sessions.append(self._auto_session)

            return sessions

        except Exception as e:
            logger.error(f"[SESSION_MANAGEMENT] Error listing sessions: {e}")
            return []

    # === Auto Session Management ===

    def session_auto(self,
                    session_id: str = "auto_session_default",
                    default_timeout: int = 720000,
                    auto_cleanup: bool = False,
                    session_prefix: str = "auto_") -> 'MCPStoreContext':
        """
        Enable automatic session mode

        In auto session mode, all tool calls are automatically routed to
        a persistent session, ensuring state continuity without manual management.

        Args:
            session_id: Auto session identifier (default: "auto_session_default")
            default_timeout: Default session timeout in seconds (default: 2 hours)
            auto_cleanup: Whether to auto-cleanup expired sessions (default: True)
            session_prefix: Prefix for auto-generated session names (default: "auto_")

        Returns:
            MCPStoreContext: Self for method chaining

        Example:
            store.for_store().session_auto()
            # Now all use_tool calls will be in the same session
            result1 = store.for_store().use_tool("browser_navigate", {"url": "https://baidu.com"})
            result2 = store.for_store().use_tool("browser_click", {"selector": "#search"})
        """
        try:
            # Save configuration
            self._auto_session_config = {
                "session_id": session_id,
                "default_timeout": default_timeout,
                "auto_cleanup": auto_cleanup,
                "session_prefix": session_prefix
            }

            # Create or get auto session
            if not self._auto_session:
                self._auto_session = self.get_session(session_id)

            # Enable auto session mode
            self._auto_session_enabled = True

            logger.info(f"[SESSION_MANAGEMENT] Auto session mode enabled with session '{session_id}'")
            return self

        except Exception as e:
            logger.error(f"[SESSION_MANAGEMENT] Failed to enable auto session mode: {e}")
            raise

    def session_manual(self) -> 'MCPStoreContext':
        """
        Switch to manual session mode

        Disables automatic session routing. Tool calls will use traditional
        mode unless explicitly called with a session.

        Returns:
            MCPStoreContext: Self for method chaining

        Example:
            store.for_store().session_manual()
            # Tool calls now use traditional mode (new connection each time)
        """
        self._auto_session_enabled = False
        logger.info("[SESSION_MANAGEMENT] Switched to manual session mode")
        return self

    def is_session_auto(self) -> bool:
        """
        Check if automatic session mode is enabled

        Returns:
            bool: True if auto session mode is active
        """
        return self._auto_session_enabled

    def current_session(self) -> Optional['Session']:
        """
        Get current auto session (if auto mode is enabled)

        Returns:
            Session: Current auto session, or None if not in auto mode

        Example:
            auto_session = store.for_store().current_session()
            if auto_session:
                auto_session.extend_session(3600)
        """
        return self._auto_session if self._auto_session_enabled else None

    # === Context Manager Support ===

    def with_session(self, session_id: str) -> 'SessionContext':
        """
        Create session context manager

        This provides automatic session lifecycle management using Python's
        context manager protocol.

        Args:
            session_id: Session identifier

        Returns:
            SessionContext: Async context manager

        Example:
            with store.for_store().with_session("browser_task") as session:
                session.bind_service("browser")
                result = session.use_tool("browser_navigate", {"url": "https://baidu.com"})
            # Session automatically closed
        """
        from .session import SessionContext
        return SessionContext(self, session_id)

    async def with_session_async(self, session_id: str) -> 'SessionContext':
        """
        Create async session context manager

        Args:
            session_id: Session identifier

        Returns:
            SessionContext: Async context manager

        Example:
            async with store.for_store().with_session_async("browser_task") as session:
                await session.bind_service_async("browser")
                result = await session.use_tool_async("browser_navigate", {"url": "https://baidu.com"})
        """
        return self.with_session(session_id)

    # === Session Management Operations ===

    def close_all_sessions(self) -> 'MCPStoreContext':
        """
        Close all sessions in current context

        Returns:
            MCPStoreContext: Self for method chaining

        Example:
            store.for_store().close_all_sessions()
        """
        try:
            # Close all cached Session objects
            for session in list(self._session_cache.values()):
                try:
                    session.close_session()
                except Exception as e:
                    logger.warning(f"[SESSION_MANAGEMENT] Error closing session {session.session_id}: {e}")

            # Clear cache
            self._session_cache.clear()

            # Close auto session
            if self._auto_session:
                try:
                    self._auto_session.close_session()
                except Exception as e:
                    logger.warning(f"[SESSION_MANAGEMENT] Error closing auto session: {e}")
                self._auto_session = None

            # Disable auto session mode
            self._auto_session_enabled = False

            logger.info("[SESSION_MANAGEMENT] All sessions closed")
            return self

        except Exception as e:
            logger.error(f"[SESSION_MANAGEMENT] Error closing all sessions: {e}")
            return self

    def cleanup_sessions(self) -> 'MCPStoreContext':
        """
        Cleanup expired sessions

        Returns:
            MCPStoreContext: Self for method chaining
        """
        try:
            # Use existing SessionManager to cleanup expired sessions
            self._store.session_manager.cleanup_expired_sessions()

            # Clean up invalid cache
            invalid_keys = []
            for key, session in self._session_cache.items():
                if not session.is_active:
                    invalid_keys.append(key)

            for key in invalid_keys:
                del self._session_cache[key]

            logger.info(f"[SESSION_MANAGEMENT] Cleaned up {len(invalid_keys)} expired session cache entries")
            return self

        except Exception as e:
            logger.error(f"[SESSION_MANAGEMENT] Error during session cleanup: {e}")
            return self

    def restart_sessions(self) -> 'MCPStoreContext':
        """
        Restart all sessions (reconnect all services)

        Returns:
            MCPStoreContext: Self for method chaining
        """
        try:
            # Restart all cached sessions
            for session in self._session_cache.values():
                try:
                    session.restart_session()
                except Exception as e:
                    logger.warning(f"[SESSION_MANAGEMENT] Error restarting session {session.session_id}: {e}")

            # Restart auto session
            if self._auto_session:
                try:
                    self._auto_session.restart_session()
                except Exception as e:
                    logger.warning(f"[SESSION_MANAGEMENT] Error restarting auto session: {e}")

            logger.info("[SESSION_MANAGEMENT] All sessions restarted")
            return self

        except Exception as e:
            logger.error(f"[SESSION_MANAGEMENT] Error restarting sessions: {e}")
            return self

    # === Enhanced Session Management Methods ===

    def find_user_session(self, user_session_id: str) -> Optional['Session']:
        """
        Find session by user-defined session ID (cross-context access)

        This is a convenience method that calls find_session with is_user_session_id=True.

        Args:
            user_session_id: User-defined session identifier

        Returns:
            Session object if found, None otherwise

        Example:
            # Access session from any context
            session = store.for_store().find_user_session("global_browser_session")
            session = store.for_agent("team_2").find_user_session("global_browser_session")
            # Both return the same session!
        """
        return self.find_session(user_session_id, is_user_session_id=True)

    def create_shared_session(self, session_id: str, shared_id: str) -> 'Session':
        """
        Create a session that can be accessed across contexts

        This is a convenience method that creates a session with a user_session_id.

        Args:
            session_id: Local session identifier
            shared_id: Global shared identifier for cross-context access

        Returns:
            Session: Created session object

        Example:
            # Create shared session in store context
            session = store.for_store().create_shared_session("browser_work", "global_browser")

            # Access from agent context
            same_session = store.for_agent("team_1").find_user_session("global_browser")
        """
        return self.create_session(session_id, user_session_id=shared_id)

    def list_agent_sessions(self) -> List['Session']:
        """
        List all sessions for current agent (Enhanced version)

        Returns:
            List of Session objects for the current agent

        Example:
            sessions = store.for_agent("team_1").list_agent_sessions()
            for session in sessions:
                print(f"Session: {session.session_id}")
        """
        try:
            sessions = []
            effective_agent_id = self._get_effective_agent_id()

            # Use enhanced SessionManager
            if hasattr(self._store.session_manager, 'list_sessions_for_agent'):
                agent_sessions_dict = self._store.session_manager.list_sessions_for_agent(effective_agent_id)

                for session_name, agent_session in agent_sessions_dict.items():
                    from .session import Session
                    session = Session(self, session_name, agent_session)
                    sessions.append(session)
            else:
                # Fallback to original logic
                agent_session = self._store.session_manager.get_session(effective_agent_id)
                if agent_session:
                    from .session import Session
                    session = Session(self, "default", agent_session)
                    sessions.append(session)

            # Include auto session if available
            if self._auto_session_enabled and self._auto_session:
                if self._auto_session not in sessions:
                    sessions.append(self._auto_session)

            return sessions

        except Exception as e:
            logger.error(f"[SESSION_MANAGEMENT] Error listing agent sessions: {e}")
            return []

    def get_session_statistics(self) -> Dict[str, Any]:
        """
        Get session statistics for current context

        Returns:
            Dictionary with session statistics

        Example:
            stats = store.for_store().get_session_statistics()
            print(f"Total sessions: {stats['total_sessions']}")
        """
        try:
            if hasattr(self._store.session_manager, 'get_session_statistics'):
                # Enhanced SessionManager statistics
                global_stats = self._store.session_manager.get_session_statistics()

                # Add context-specific information
                effective_agent_id = self._get_effective_agent_id()
                agent_sessions = self.list_agent_sessions()

                context_stats = {
                    "context_type": "store" if self._context_type.name == "STORE" else "agent",
                    "agent_id": effective_agent_id,
                    "context_sessions": len(agent_sessions),
                    "auto_session_enabled": self._auto_session_enabled,
                    "cached_session_objects": len(self._session_cache)
                }

                return {**global_stats, "context_info": context_stats}
            else:
                # Basic statistics for original SessionManager
                agent_sessions = self.list_agent_sessions()
                return {
                    "context_sessions": len(agent_sessions),
                    "auto_session_enabled": self._auto_session_enabled,
                    "cached_session_objects": len(self._session_cache)
                }

        except Exception as e:
            logger.error(f"[SESSION_MANAGEMENT] Error getting session statistics: {e}")
            return {"error": str(e)}

    def register_session_globally(self, session_id: str, global_id: str) -> bool:
        """
        Register an existing session for global access

        Args:
            session_id: Local session identifier
            global_id: Global identifier for cross-context access

        Returns:
            bool: True if registration successful, False otherwise

        Example:
            # Create local session
            session = store.for_store().create_session("browser_work")

            # Register for global access
            success = store.for_store().register_session_globally("browser_work", "shared_browser")

            # Now accessible globally
            same_session = store.for_agent("team_1").find_user_session("shared_browser")
        """
        try:
            effective_agent_id = self._get_effective_agent_id()

            if hasattr(self._store.session_manager, 'register_user_session'):
                success = self._store.session_manager.register_user_session(
                    global_id, effective_agent_id, session_id
                )

                if success:
                    # Update local cache
                    session = self.find_session(session_id)
                    if session:
                        self._session_cache[f"user:{global_id}"] = session

                return success
            else:
                logger.warning("[SESSION_MANAGEMENT] Global session registration not supported by current SessionManager")
                return False

        except Exception as e:
            logger.error(f"[SESSION_MANAGEMENT] Error registering session globally: {e}")
            return False

    # === LangChain Integration ===

    def for_langchain_with_session(self, session_id: str, create_if_not_exists: bool = True) -> 'SessionAwareLangChainAdapter':
        """
        Create a session-aware LangChain adapter

        This method creates LangChain tools that are bound to a specific session,
        ensuring state persistence across multiple tool calls in LangChain workflows.

        Args:
            session_id: Session identifier
            create_if_not_exists: Whether to create session if it doesn't exist (default: True)

        Returns:
            SessionAwareLangChainAdapter: Session-bound LangChain adapter

        Example:
            # Create session-bound LangChain tools
            session_adapter = store.for_store().for_langchain_with_session("browser_session")
            tools = session_adapter.list_tools()

            # Use with LangChain agent - browser state will persist!
            agent = create_react_agent(llm, tools)
            result = agent.invoke({"messages": [HumanMessage("打开百度，然后搜索天气")]})
        """
        try:
            # Get or create session
            session = self.find_session(session_id)
            if not session and create_if_not_exists:
                session = self.create_session(session_id)
            elif not session:
                raise ValueError(f"Session '{session_id}' not found and create_if_not_exists=False")

            # Create session-aware adapter
            # Note: 这是一个桥接方法，存在 core → adapters 的向上依赖
            # 但为了 API 便利性保留，使用延迟导入减少影响
            from mcpstore.adapters.langchain_adapter import SessionAwareLangChainAdapter
            adapter = SessionAwareLangChainAdapter(self, session)

            logger.info(f"[SESSION_MANAGEMENT] Created session-aware LangChain adapter for session '{session_id}'")
            return adapter

        except Exception as e:
            logger.error(f"[SESSION_MANAGEMENT] Error creating session-aware LangChain adapter: {e}")
            raise

    def for_langchain_with_auto_session(self) -> 'SessionAwareLangChainAdapter':
        """
        Create a LangChain adapter using the current auto session

        This is a convenience method for using the auto session with LangChain.
        Auto session mode must be enabled first.

        Returns:
            SessionAwareLangChainAdapter: Auto session-bound LangChain adapter

        Example:
            # Enable auto session mode
            store.for_store().session_auto()

            # Create LangChain tools bound to auto session
            session_adapter = store.for_store().for_langchain_with_auto_session()
            tools = session_adapter.list_tools()

            # All tool calls will automatically use the same session
            agent = create_react_agent(llm, tools)
        """
        if not self._auto_session_enabled or not self._auto_session:
            raise RuntimeError("Auto session mode is not enabled. Call session_auto() first.")

        # Note: 桥接方法，延迟导入减少向上依赖影响
        from mcpstore.adapters.langchain_adapter import SessionAwareLangChainAdapter
        adapter = SessionAwareLangChainAdapter(self, self._auto_session)

        logger.info("[SESSION_MANAGEMENT] Created LangChain adapter for auto session")
        return adapter

    def for_langchain_with_shared_session(self, shared_id: str) -> 'SessionAwareLangChainAdapter':
        """
        Create a LangChain adapter using a shared session (cross-context access)

        Args:
            shared_id: Shared session identifier

        Returns:
            SessionAwareLangChainAdapter: Shared session-bound LangChain adapter

        Example:
            # Access shared session from any context
            session_adapter = store.for_store().for_langchain_with_shared_session("global_browser")
            session_adapter = store.for_agent("team_1").for_langchain_with_shared_session("global_browser")
            # Both return tools bound to the same session!
        """
        try:
            session = self.find_user_session(shared_id)
            if not session:
                raise ValueError(f"Shared session '{shared_id}' not found")

            # Note: 桥接方法，延迟导入减少向上依赖影响
            from mcpstore.adapters.langchain_adapter import SessionAwareLangChainAdapter
            adapter = SessionAwareLangChainAdapter(self, session)

            logger.info(f"[SESSION_MANAGEMENT] Created LangChain adapter for shared session '{shared_id}'")
            return adapter

        except Exception as e:
            logger.error(f"[SESSION_MANAGEMENT] Error creating LangChain adapter for shared session: {e}")
            raise

    # === Internal Helper Methods ===

    def _get_effective_agent_id(self) -> str:
        """
        Get effective agent ID for current context

        Returns:
            str: Agent ID to use for session operations
        """
        if self._context_type == ContextType.STORE:
            # Store 上下文使用 global_agent_store_id
            return self._store.client_manager.global_agent_store_id
        else:
            # Agent 上下文使用实际的 agent_id
            return self._agent_id

    def _use_tool_with_session(self, tool_name: str, args: Dict[str, Any] = None, **kwargs) -> Any:
        """
        Internal method to execute tool with automatic session

        This method is called when auto session mode is enabled.
        It routes tool execution to the auto session.

        Args:
            tool_name: Tool name
            args: Tool arguments
            **kwargs: Additional arguments

        Returns:
            Tool execution result
        """
        if not self._auto_session:
            raise RuntimeError("Auto session not initialized")

        logger.debug(f"[SESSION_MANAGEMENT] Routing tool '{tool_name}' to auto session")
        # Avoid passing duplicate session_id when routing to session API
        kwargs.pop('session_id', None)
        return self._auto_session.use_tool(tool_name, args, **kwargs)  # return_extracted   - propagated by callers

    async def _use_tool_with_session_async(self, tool_name: str, args: Dict[str, Any] = None, **kwargs) -> Any:
        """
        Internal async method to execute tool with automatic session

        This method routes tool execution through the session-aware path by creating
        a ToolExecutionRequest with session_id and calling the store's process_tool_request.
        """
        if not self._auto_session:
            raise RuntimeError("Auto session not initialized")

        logger.debug(f"[SESSION_MANAGEMENT] Routing tool '{tool_name}' to auto session (async)")

        # 使用 Session 的 use_tool_async 方法，它会直接使用缓存的 FastMCP Client
        # Avoid duplicate session_id when delegating to Session API
        kwargs.pop('session_id', None)
        return await self._auto_session.use_tool_async(tool_name, args, **kwargs)
