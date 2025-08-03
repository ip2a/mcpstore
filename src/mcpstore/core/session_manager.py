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
    """Session manager"""
    def __init__(self, session_timeout: int = 3600):
        self.sessions: Dict[str, AgentSession] = {}
        self.session_timeout = timedelta(seconds=session_timeout)
        
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
