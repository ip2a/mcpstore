"""
FastMCP Authentication Types and Data Models
认证相关的类型定义和数据模型 - 专门用于FastMCP集成
"""

from enum import Enum
from typing import Dict, Any, List, Optional

from pydantic import BaseModel, Field


class AuthProviderType(str, Enum):
    """FastMCP支持的认证提供者类型"""
    BEARER = "bearer"
    OAUTH = "oauth"
    GOOGLE = "google"
    GITHUB = "github"
    WORKOS = "workos"
    CUSTOM = "custom"


class AuthProviderConfig(BaseModel):
    """FastMCP认证提供者配置"""
    provider_type: AuthProviderType = Field(..., description="认证提供者类型")
    config: Dict[str, Any] = Field(default_factory=dict, description="提供者特定配置")
    enabled: bool = Field(True, description="是否启用")
    
    # Bearer Token 特定配置
    jwks_uri: Optional[str] = Field(None, description="JWKS URI")
    issuer: Optional[str] = Field(None, description="JWT Issuer")
    audience: Optional[str] = Field(None, description="JWT Audience")
    algorithm: Optional[str] = Field("RS256", description="JWT算法")
    
    # OAuth 特定配置
    client_id: Optional[str] = Field(None, description="OAuth客户端ID")
    client_secret: Optional[str] = Field(None, description="OAuth客户端密钥")
    base_url: Optional[str] = Field(None, description="服务器基础URL")
    redirect_path: Optional[str] = Field("/auth/callback", description="OAuth回调路径")
    required_scopes: List[str] = Field(default_factory=list, description="必需的权限范围")


class FastMCPAuthConfig(BaseModel):
    """生成给FastMCP的认证配置"""
    provider_class: str = Field(..., description="FastMCP认证提供者类名")
    config_params: Dict[str, Any] = Field(default_factory=dict, description="配置参数")
    import_path: str = Field(..., description="导入路径")
    
    @classmethod
    def for_bearer_token(cls, jwks_uri: str, issuer: str, audience: str, algorithm: str = "RS256") -> 'FastMCPAuthConfig':
        """创建Bearer Token认证配置"""
        return cls(
            provider_class="BearerAuthProvider",
            import_path="fastmcp.server.auth",
            config_params={
                "jwks_uri": jwks_uri,
                "issuer": issuer,
                "audience": audience,
                "algorithm": algorithm
            }
        )
    
    @classmethod
    def for_google_oauth(cls, client_id: str, client_secret: str, base_url: str, 
                        required_scopes: List[str] = None) -> 'FastMCPAuthConfig':
        """创建Google OAuth认证配置"""
        return cls(
            provider_class="GoogleProvider",
            import_path="fastmcp.server.auth.providers.google",
            config_params={
                "client_id": client_id,
                "client_secret": client_secret,
                "base_url": base_url,
                "required_scopes": required_scopes or ["openid", "email", "profile"]
            }
        )
    
    @classmethod
    def for_github_oauth(cls, client_id: str, client_secret: str, base_url: str,
                        required_scopes: List[str] = None) -> 'FastMCPAuthConfig':
        """创建GitHub OAuth认证配置"""
        return cls(
            provider_class="GitHubProvider",
            import_path="fastmcp.server.auth.providers.github",
            config_params={
                "client_id": client_id,
                "client_secret": client_secret,
                "base_url": base_url,
                "required_scopes": required_scopes or ["read:user", "user:email"]
            }
        )
    
    @classmethod
    def for_workos_oauth(cls, authkit_domain: str, base_url: str) -> 'FastMCPAuthConfig':
        """创建WorkOS OAuth认证配置"""
        return cls(
            provider_class="AuthKitProvider",
            import_path="fastmcp.server.auth.providers.workos", 
            config_params={
                "authkit_domain": authkit_domain,
                "base_url": base_url
            }
        )


class HubAuthConfig(BaseModel):
    """Hub认证配置"""
    hub_name: str = Field(..., description="Hub名称")
    auth_enabled: bool = Field(False, description="是否启用认证")
    auth_provider: Optional[AuthProviderConfig] = Field(None, description="认证提供者")
    required_scopes: List[str] = Field(default_factory=list, description="必需的权限范围")
    protected_tools: List[str] = Field(default_factory=list, description="受保护的工具")
    public_tools: List[str] = Field(default_factory=list, description="公开的工具")


class JWTPayloadConfig(BaseModel):
    """JWT Payload配置 - 用于FastMCP token生成"""
    client_id: str = Field(..., description="客户端ID")
    scopes: List[str] = Field(default_factory=list, description="权限范围")
    custom_claims: Dict[str, Any] = Field(default_factory=dict, description="自定义声明")
    expires_in: int = Field(3600, description="过期时间（秒）")
