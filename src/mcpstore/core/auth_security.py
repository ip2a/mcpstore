#!/usr/bin/env python3
"""
认证与安全功能
Bearer token 认证、OAuth 2.1 集成、API 密钥管理、基于角色的访问控制
"""

import logging
import secrets
import hashlib
import time
from typing import Dict, List, Set, Any, Optional, Union, Callable, Tuple
from dataclasses import dataclass, field
from enum import Enum
import json
from pathlib import Path
import base64
from datetime import datetime, timedelta

logger = logging.getLogger(__name__)

class AuthType(Enum):
    """认证类型"""
    BEARER_TOKEN = "bearer_token"
    API_KEY = "api_key"
    OAUTH2 = "oauth2"
    BASIC_AUTH = "basic_auth"
    CUSTOM = "custom"

class Permission(Enum):
    """权限类型"""
    READ = "read"
    WRITE = "write"
    EXECUTE = "execute"
    ADMIN = "admin"
    DELETE = "delete"

@dataclass
class Role:
    """角色定义"""
    name: str
    permissions: Set[Permission] = field(default_factory=set)
    allowed_services: Set[str] = field(default_factory=set)  # 允许访问的服务
    allowed_tools: Set[str] = field(default_factory=set)     # 允许使用的工具
    blocked_tools: Set[str] = field(default_factory=set)     # 禁止使用的工具
    description: Optional[str] = None
    expires_at: Optional[datetime] = None

@dataclass
class User:
    """用户定义"""
    username: str
    user_id: str
    roles: Set[str] = field(default_factory=set)
    api_keys: Dict[str, str] = field(default_factory=dict)  # key_name -> hashed_key
    oauth_tokens: Dict[str, Any] = field(default_factory=dict)
    created_at: datetime = field(default_factory=datetime.now)
    last_login: Optional[datetime] = None
    active: bool = True
    metadata: Dict[str, Any] = field(default_factory=dict)

@dataclass
class AuthConfig:
    """认证配置"""
    auth_type: AuthType
    config: Dict[str, Any] = field(default_factory=dict)
    enabled: bool = True

class TokenManager:
    """令牌管理器"""
    
    def __init__(self):
        self._tokens: Dict[str, Dict[str, Any]] = {}  # token -> token_info
        self._token_expiry: Dict[str, datetime] = {}
    
    def generate_bearer_token(self, user_id: str, expires_in: int = 3600) -> str:
        """生成 Bearer Token"""
        token = secrets.token_urlsafe(32)
        expires_at = datetime.now() + timedelta(seconds=expires_in)
        
        self._tokens[token] = {
            "user_id": user_id,
            "type": "bearer",
            "created_at": datetime.now(),
            "expires_at": expires_at,
            "scopes": []
        }
        self._token_expiry[token] = expires_at
        
        logger.info(f"Generated bearer token for user {user_id}")
        return token
    
    def generate_api_key(self, user_id: str, key_name: str) -> Tuple[str, str]:
        """生成 API Key"""
        # 生成原始密钥
        raw_key = f"mcp_{secrets.token_urlsafe(32)}"
        
        # 生成哈希
        key_hash = hashlib.sha256(raw_key.encode()).hexdigest()
        
        # 存储
        token_id = f"api_{secrets.token_urlsafe(16)}"
        self._tokens[token_id] = {
            "user_id": user_id,
            "type": "api_key",
            "key_name": key_name,
            "key_hash": key_hash,
            "created_at": datetime.now(),
            "last_used": None
        }
        
        logger.info(f"Generated API key '{key_name}' for user {user_id}")
        return raw_key, token_id
    
    def validate_token(self, token: str) -> Optional[Dict[str, Any]]:
        """验证令牌"""
        # 检查是否是 Bearer Token
        if token in self._tokens:
            token_info = self._tokens[token]
            
            # 检查过期时间
            if token in self._token_expiry:
                if datetime.now() > self._token_expiry[token]:
                    self.revoke_token(token)
                    return None
            
            return token_info
        
        # 检查是否是 API Key
        for token_id, token_info in self._tokens.items():
            if token_info.get("type") == "api_key":
                key_hash = hashlib.sha256(token.encode()).hexdigest()
                if key_hash == token_info.get("key_hash"):
                    # 更新最后使用时间
                    token_info["last_used"] = datetime.now()
                    return token_info
        
        return None
    
    def revoke_token(self, token: str):
        """撤销令牌"""
        if token in self._tokens:
            del self._tokens[token]
        if token in self._token_expiry:
            del self._token_expiry[token]
        logger.info(f"Revoked token: {token[:8]}...")
    
    def cleanup_expired_tokens(self):
        """清理过期令牌"""
        now = datetime.now()
        expired_tokens = [
            token for token, expires_at in self._token_expiry.items()
            if now > expires_at
        ]
        
        for token in expired_tokens:
            self.revoke_token(token)
        
        if expired_tokens:
            logger.info(f"Cleaned up {len(expired_tokens)} expired tokens")

class RoleManager:
    """角色管理器"""
    
    def __init__(self):
        self._roles: Dict[str, Role] = {}
        self._create_default_roles()
    
    def _create_default_roles(self):
        """创建默认角色"""
        # 管理员角色
        admin_role = Role(
            name="admin",
            permissions={Permission.READ, Permission.WRITE, Permission.EXECUTE, Permission.ADMIN, Permission.DELETE},
            description="Full access to all resources"
        )
        self._roles["admin"] = admin_role
        
        # 用户角色
        user_role = Role(
            name="user",
            permissions={Permission.READ, Permission.EXECUTE},
            description="Standard user with read and execute permissions"
        )
        self._roles["user"] = user_role
        
        # 只读角色
        readonly_role = Role(
            name="readonly",
            permissions={Permission.READ},
            description="Read-only access"
        )
        self._roles["readonly"] = readonly_role
        
        # 开发者角色
        developer_role = Role(
            name="developer",
            permissions={Permission.READ, Permission.WRITE, Permission.EXECUTE},
            description="Developer access with read, write, and execute permissions"
        )
        self._roles["developer"] = developer_role
    
    def create_role(self, role: Role):
        """创建角色"""
        self._roles[role.name] = role
        logger.info(f"Created role: {role.name}")
    
    def get_role(self, role_name: str) -> Optional[Role]:
        """获取角色"""
        return self._roles.get(role_name)
    
    def list_roles(self) -> List[str]:
        """列出所有角色"""
        return list(self._roles.keys())
    
    def check_permission(self, role_names: Set[str], permission: Permission) -> bool:
        """检查权限"""
        for role_name in role_names:
            role = self._roles.get(role_name)
            if role and permission in role.permissions:
                return True
        return False
    
    def check_tool_access(self, role_names: Set[str], tool_name: str, service_name: str) -> bool:
        """检查工具访问权限"""
        for role_name in role_names:
            role = self._roles.get(role_name)
            if not role:
                continue
            
            # 检查是否在禁止列表中
            if tool_name in role.blocked_tools:
                return False
            
            # 检查是否在允许列表中（如果列表不为空）
            if role.allowed_tools and tool_name not in role.allowed_tools:
                continue
            
            # 检查服务访问权限
            if role.allowed_services and service_name not in role.allowed_services:
                continue
            
            return True
        
        return False

class UserManager:
    """用户管理器"""
    
    def __init__(self):
        self._users: Dict[str, User] = {}
        self._username_to_id: Dict[str, str] = {}
    
    def create_user(self, username: str, roles: List[str] = None) -> str:
        """创建用户"""
        user_id = f"user_{secrets.token_urlsafe(16)}"
        user = User(
            username=username,
            user_id=user_id,
            roles=set(roles or ["user"])
        )
        
        self._users[user_id] = user
        self._username_to_id[username] = user_id
        
        logger.info(f"Created user: {username} ({user_id})")
        return user_id
    
    def get_user(self, user_id: str) -> Optional[User]:
        """获取用户"""
        return self._users.get(user_id)
    
    def get_user_by_username(self, username: str) -> Optional[User]:
        """通过用户名获取用户"""
        user_id = self._username_to_id.get(username)
        if user_id:
            return self._users.get(user_id)
        return None
    
    def update_user_roles(self, user_id: str, roles: List[str]):
        """更新用户角色"""
        user = self._users.get(user_id)
        if user:
            user.roles = set(roles)
            logger.info(f"Updated roles for user {user_id}: {roles}")
    
    def deactivate_user(self, user_id: str):
        """停用用户"""
        user = self._users.get(user_id)
        if user:
            user.active = False
            logger.info(f"Deactivated user: {user_id}")

class AuthenticationManager:
    """认证管理器"""
    
    def __init__(self):
        self.token_manager = TokenManager()
        self.role_manager = RoleManager()
        self.user_manager = UserManager()
        self._auth_configs: Dict[str, AuthConfig] = {}
    
    def setup_bearer_auth(self, enabled: bool = True):
        """设置 Bearer Token 认证"""
        config = AuthConfig(
            auth_type=AuthType.BEARER_TOKEN,
            enabled=enabled
        )
        self._auth_configs["bearer"] = config
        logger.info(f"Bearer token authentication {'enabled' if enabled else 'disabled'}")
    
    def setup_api_key_auth(self, enabled: bool = True):
        """设置 API Key 认证"""
        config = AuthConfig(
            auth_type=AuthType.API_KEY,
            enabled=enabled
        )
        self._auth_configs["api_key"] = config
        logger.info(f"API key authentication {'enabled' if enabled else 'disabled'}")
    
    def authenticate_request(self, auth_header: str) -> Optional[Dict[str, Any]]:
        """认证请求"""
        if not auth_header:
            return None
        
        # Bearer Token
        if auth_header.startswith("Bearer "):
            token = auth_header[7:]
            token_info = self.token_manager.validate_token(token)
            if token_info:
                user = self.user_manager.get_user(token_info["user_id"])
                if user and user.active:
                    return {
                        "user": user,
                        "token_info": token_info,
                        "auth_type": "bearer"
                    }
        
        # API Key
        elif auth_header.startswith("ApiKey "):
            api_key = auth_header[7:]
            token_info = self.token_manager.validate_token(api_key)
            if token_info:
                user = self.user_manager.get_user(token_info["user_id"])
                if user and user.active:
                    return {
                        "user": user,
                        "token_info": token_info,
                        "auth_type": "api_key"
                    }
        
        return None
    
    def check_tool_permission(self, auth_info: Dict[str, Any], tool_name: str, service_name: str) -> bool:
        """检查工具使用权限"""
        if not auth_info:
            return False
        
        user = auth_info["user"]
        
        # 检查用户是否激活
        if not user.active:
            return False
        
        # 检查角色权限
        return self.role_manager.check_tool_access(user.roles, tool_name, service_name)
    
    def create_user_with_api_key(self, username: str, key_name: str, roles: List[str] = None) -> Tuple[str, str]:
        """创建用户并生成 API Key"""
        user_id = self.user_manager.create_user(username, roles)
        api_key, key_id = self.token_manager.generate_api_key(user_id, key_name)
        return api_key, user_id
    
    def get_auth_summary(self) -> Dict[str, Any]:
        """获取认证摘要"""
        return {
            "enabled_auth_types": [
                config.auth_type.value for config in self._auth_configs.values()
                if config.enabled
            ],
            "total_users": len(self.user_manager._users),
            "active_users": len([u for u in self.user_manager._users.values() if u.active]),
            "total_roles": len(self.role_manager._roles),
            "active_tokens": len(self.token_manager._tokens)
        }

# 全局实例
_global_auth_manager = None

def get_auth_manager() -> AuthenticationManager:
    """获取全局认证管理器"""
    global _global_auth_manager
    if _global_auth_manager is None:
        _global_auth_manager = AuthenticationManager()
    return _global_auth_manager
