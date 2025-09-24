import logging
import uuid
from datetime import datetime, timedelta
from typing import Dict, Any, Optional

from fastmcp import Client

logger = logging.getLogger(__name__)

class AgentSession:
    """Agent session class"""
    def __init__(self, agent_id: str):
        self.agent_id = agent_id
        self.services: Dict[str, Client] = {}  # service_name -> Client
        self.tools: Dict[str, Dict[str, Any]] = {}  # tool_name -> tool_info
        self.last_active = datetime.now()
        self.created_at = datetime.now()
        
    def update_activity(self):
        """Update last activity time"""
        self.last_active = datetime.now()
        
    def add_service(self, service_name: str, client: Client):
        """Add service"""
        self.services[service_name] = client
        
    def add_tool(self, tool_name: str, tool_info: Dict[str, Any], service_name: str):
        """Add tool"""
        self.tools[tool_name] = {
            **tool_info,
            "service_name": service_name
        }
        
    def get_service_for_tool(self, tool_name: str) -> Optional[str]:
        """Get service name corresponding to tool"""
        return self.tools.get(tool_name, {}).get("service_name")
        
    def get_all_tools(self) -> Dict[str, Dict[str, Any]]:
        """Get all tool information"""
        return self.tools

class SessionManager:
    """
    Enhanced Session manager with multi-session and cross-context support
    
    This enhanced version maintains full backward compatibility while adding:
    - Multiple named sessions per agent
    - User-defined session IDs with cross-context access
    - Session mapping and discovery capabilities
    """
    def __init__(self, session_timeout: int = 3600):
        # 🎯 Original storage (backward compatibility)
        self.sessions: Dict[str, AgentSession] = {}
        self.session_timeout = timedelta(seconds=session_timeout)
        
        # 🆕 Enhanced storage for multi-session support
        # Format: {agent_id: {session_name: AgentSession}}
        self.named_sessions: Dict[str, Dict[str, AgentSession]] = {}
        
        # 🆕 User session mapping for cross-context access
        # Format: {user_session_id: (agent_id, session_name)}
        self.user_session_mapping: Dict[str, tuple[str, str]] = {}
        
        # 🆕 Global session registry for cross-context discovery
        # Format: {global_session_id: (agent_id, session_name)}
        self.global_session_registry: Dict[str, tuple[str, str]] = {}
        
    def create_session(self, agent_id: Optional[str] = None) -> AgentSession:
        """Create new session"""
        if not agent_id:
            agent_id = str(uuid.uuid4())
            
        session = AgentSession(agent_id)
        self.sessions[agent_id] = session
        logger.info(f"Created new session for agent {agent_id}")
        return session
        
    def get_session(self, agent_id: str) -> Optional[AgentSession]:
        """获取会话"""
        session = self.sessions.get(agent_id)
        if session:
            # 检查会话是否过期
            if datetime.now() - session.last_active > self.session_timeout:
                logger.info(f"Session expired for agent {agent_id}")
                del self.sessions[agent_id]
                return None
            session.update_activity()
        return session
        
    def get_or_create_session(self, agent_id: Optional[str] = None) -> AgentSession:
        """获取或创建会话"""
        if agent_id and (session := self.get_session(agent_id)):
            return session
        return self.create_session(agent_id)
        
    def cleanup_expired_sessions(self):
        """清理过期会话"""
        now = datetime.now()
        expired = [
            agent_id for agent_id, session in self.sessions.items()
            if now - session.last_active > self.session_timeout
        ]
        for agent_id in expired:
            del self.sessions[agent_id]
            logger.info(f"Cleaned up expired session for agent {agent_id}")

    # === Enhanced Multi-Session Support ===
    
    def create_named_session(self, agent_id: str, session_name: str, user_session_id: Optional[str] = None) -> AgentSession:
        """
        Create a named session for an agent
        
        This allows multiple sessions per agent, each with a unique name.
        
        Args:
            agent_id: Agent identifier
            session_name: Unique session name within the agent's scope
            user_session_id: Optional user-defined session ID for cross-context access
            
        Returns:
            AgentSession: Created session object
            
        Example:
            # Create multiple sessions for the same agent
            browser_session = session_manager.create_named_session("team_1", "browser_work")
            api_session = session_manager.create_named_session("team_1", "api_calls")
        """
        try:
            # 🎯 Initialize agent's session dictionary if not exists
            if agent_id not in self.named_sessions:
                self.named_sessions[agent_id] = {}
            
            # 🎯 Check if session name already exists for this agent
            if session_name in self.named_sessions[agent_id]:
                logger.warning(f"Session '{session_name}' already exists for agent '{agent_id}', returning existing session")
                return self.named_sessions[agent_id][session_name]
            
            # 🎯 Create new AgentSession
            session = AgentSession(agent_id)
            
            # 🎯 Store in named sessions
            self.named_sessions[agent_id][session_name] = session
            
            # 🎯 Register user session mapping if provided
            if user_session_id:
                if user_session_id in self.user_session_mapping:
                    logger.warning(f"User session ID '{user_session_id}' already exists, overwriting")
                self.user_session_mapping[user_session_id] = (agent_id, session_name)
                
                # 🎯 Also register in global registry
                self.global_session_registry[user_session_id] = (agent_id, session_name)
            
            logger.info(f"Created named session '{session_name}' for agent '{agent_id}'" + 
                       (f" with user session ID '{user_session_id}'" if user_session_id else ""))
            return session
            
        except Exception as e:
            logger.error(f"Failed to create named session '{session_name}' for agent '{agent_id}': {e}")
            raise
    
    def get_named_session(self, agent_id: str, session_name: str) -> Optional[AgentSession]:
        """
        Get a named session for an agent
        
        Args:
            agent_id: Agent identifier
            session_name: Session name
            
        Returns:
            AgentSession if found and not expired, None otherwise
        """
        try:
            # 🎯 Check if agent has any named sessions
            if agent_id not in self.named_sessions:
                return None
            
            # 🎯 Check if specific session exists
            session = self.named_sessions[agent_id].get(session_name)
            if not session:
                return None
            
            # 🎯 Check expiration
            if datetime.now() - session.last_active > self.session_timeout:
                logger.info(f"Named session '{session_name}' expired for agent '{agent_id}'")
                del self.named_sessions[agent_id][session_name]
                # Clean up empty agent entry
                if not self.named_sessions[agent_id]:
                    del self.named_sessions[agent_id]
                return None
            
            # 🎯 Update activity and return
            session.update_activity()
            return session
            
        except Exception as e:
            logger.error(f"Error getting named session '{session_name}' for agent '{agent_id}': {e}")
            return None
    
    def get_session_by_user_id(self, user_session_id: str) -> Optional[AgentSession]:
        """
        Get session by user-defined session ID (cross-context access)
        
        This allows accessing sessions across different contexts using a
        user-defined identifier.
        
        Args:
            user_session_id: User-defined session identifier
            
        Returns:
            AgentSession if found and not expired, None otherwise
            
        Example:
            # Access session from any context
            session = session_manager.get_session_by_user_id("shared_browser_session")
        """
        try:
            # 🎯 Look up in user session mapping
            if user_session_id not in self.user_session_mapping:
                return None
            
            agent_id, session_name = self.user_session_mapping[user_session_id]
            
            # 🎯 Get the actual session
            session = self.get_named_session(agent_id, session_name)
            
            # 🎯 Clean up mapping if session expired
            if not session:
                del self.user_session_mapping[user_session_id]
                if user_session_id in self.global_session_registry:
                    del self.global_session_registry[user_session_id]
            
            return session
            
        except Exception as e:
            logger.error(f"Error getting session by user ID '{user_session_id}': {e}")
            return None
    
    def list_sessions_for_agent(self, agent_id: str) -> Dict[str, AgentSession]:
        """
        List all sessions for an agent
        
        Args:
            agent_id: Agent identifier
            
        Returns:
            Dictionary of session_name -> AgentSession
        """
        try:
            if agent_id not in self.named_sessions:
                return {}
            
            # 🎯 Filter out expired sessions
            valid_sessions = {}
            expired_sessions = []
            
            for session_name, session in self.named_sessions[agent_id].items():
                if datetime.now() - session.last_active <= self.session_timeout:
                    valid_sessions[session_name] = session
                    session.update_activity()
                else:
                    expired_sessions.append(session_name)
            
            # 🎯 Clean up expired sessions
            for session_name in expired_sessions:
                del self.named_sessions[agent_id][session_name]
                logger.info(f"Cleaned up expired named session '{session_name}' for agent '{agent_id}'")
            
            # 🎯 Clean up empty agent entry
            if not self.named_sessions[agent_id]:
                del self.named_sessions[agent_id]
            
            return valid_sessions
            
        except Exception as e:
            logger.error(f"Error listing sessions for agent '{agent_id}': {e}")
            return {}
    
    def list_all_user_sessions(self) -> Dict[str, tuple[str, str]]:
        """
        List all user-defined sessions with their mappings
        
        Returns:
            Dictionary of user_session_id -> (agent_id, session_name)
        """
        # 🎯 Clean up expired mappings first
        expired_user_sessions = []
        
        for user_session_id, (agent_id, session_name) in self.user_session_mapping.items():
            session = self.get_named_session(agent_id, session_name)
            if not session:
                expired_user_sessions.append(user_session_id)
        
        # 🎯 Remove expired mappings
        for user_session_id in expired_user_sessions:
            del self.user_session_mapping[user_session_id]
            if user_session_id in self.global_session_registry:
                del self.global_session_registry[user_session_id]
        
        return dict(self.user_session_mapping)
    
    def register_user_session(self, user_session_id: str, agent_id: str, session_name: str) -> bool:
        """
        Register an existing named session with a user-defined ID
        
        Args:
            user_session_id: User-defined session identifier
            agent_id: Agent identifier
            session_name: Existing session name
            
        Returns:
            bool: True if registration successful, False otherwise
        """
        try:
            # 🎯 Verify the session exists
            session = self.get_named_session(agent_id, session_name)
            if not session:
                logger.error(f"Cannot register user session '{user_session_id}': session '{session_name}' not found for agent '{agent_id}'")
                return False
            
            # 🎯 Check for conflicts
            if user_session_id in self.user_session_mapping:
                existing_agent_id, existing_session_name = self.user_session_mapping[user_session_id]
                logger.warning(f"User session ID '{user_session_id}' already maps to ({existing_agent_id}, {existing_session_name}), overwriting")
            
            # 🎯 Register mapping
            self.user_session_mapping[user_session_id] = (agent_id, session_name)
            self.global_session_registry[user_session_id] = (agent_id, session_name)
            
            logger.info(f"Registered user session '{user_session_id}' -> ({agent_id}, {session_name})")
            return True
            
        except Exception as e:
            logger.error(f"Error registering user session '{user_session_id}': {e}")
            return False
    
    def unregister_user_session(self, user_session_id: str) -> bool:
        """
        Unregister a user-defined session ID
        
        This removes the mapping but does not delete the underlying session.
        
        Args:
            user_session_id: User-defined session identifier
            
        Returns:
            bool: True if unregistration successful, False if not found
        """
        try:
            if user_session_id not in self.user_session_mapping:
                logger.warning(f"User session ID '{user_session_id}' not found for unregistration")
                return False
            
            # 🎯 Remove mappings
            del self.user_session_mapping[user_session_id]
            if user_session_id in self.global_session_registry:
                del self.global_session_registry[user_session_id]
            
            logger.info(f"Unregistered user session '{user_session_id}'")
            return True
            
        except Exception as e:
            logger.error(f"Error unregistering user session '{user_session_id}': {e}")
            return False
    
    def cleanup_all_expired_sessions(self):
        """
        Enhanced cleanup that handles both original and named sessions
        """
        try:
            # 🎯 Clean up original sessions (backward compatibility)
            self.cleanup_expired_sessions()
            
            # 🎯 Clean up named sessions
            now = datetime.now()
            agents_to_clean = []
            
            for agent_id, sessions_dict in self.named_sessions.items():
                expired_sessions = []
                
                for session_name, session in sessions_dict.items():
                    if now - session.last_active > self.session_timeout:
                        expired_sessions.append(session_name)
                
                # Remove expired sessions
                for session_name in expired_sessions:
                    del sessions_dict[session_name]
                    logger.info(f"Cleaned up expired named session '{session_name}' for agent '{agent_id}'")
                
                # Mark agent for cleanup if no sessions left
                if not sessions_dict:
                    agents_to_clean.append(agent_id)
            
            # Clean up empty agent entries
            for agent_id in agents_to_clean:
                del self.named_sessions[agent_id]
            
            # 🎯 Clean up orphaned user session mappings
            orphaned_user_sessions = []
            for user_session_id, (agent_id, session_name) in self.user_session_mapping.items():
                if agent_id not in self.named_sessions or session_name not in self.named_sessions.get(agent_id, {}):
                    orphaned_user_sessions.append(user_session_id)
            
            for user_session_id in orphaned_user_sessions:
                del self.user_session_mapping[user_session_id]
                if user_session_id in self.global_session_registry:
                    del self.global_session_registry[user_session_id]
                logger.info(f"Cleaned up orphaned user session mapping '{user_session_id}'")
            
            logger.info("Enhanced session cleanup completed")
            
        except Exception as e:
            logger.error(f"Error during enhanced session cleanup: {e}")

    def get_session_statistics(self) -> Dict[str, Any]:
        """
        Get comprehensive session statistics
        
        Returns:
            Dictionary with session statistics
        """
        try:
            # Count original sessions
            original_sessions = len(self.sessions)
            
            # Count named sessions
            total_named_sessions = 0
            agents_with_named_sessions = 0
            for agent_sessions in self.named_sessions.values():
                if agent_sessions:
                    agents_with_named_sessions += 1
                    total_named_sessions += len(agent_sessions)
            
            # Count user mappings
            user_mappings = len(self.user_session_mapping)
            
            return {
                "original_sessions": original_sessions,
                "named_sessions": {
                    "total": total_named_sessions,
                    "agents_with_sessions": agents_with_named_sessions,
                    "average_per_agent": total_named_sessions / max(agents_with_named_sessions, 1)
                },
                "user_mappings": user_mappings,
                "total_sessions": original_sessions + total_named_sessions,
                "session_timeout_seconds": int(self.session_timeout.total_seconds())
            }
            
        except Exception as e:
            logger.error(f"Error getting session statistics: {e}")
            return {"error": str(e)}
