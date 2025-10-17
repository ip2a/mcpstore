"""
FastMCP Authentication Configuration Builders
认证配置构建器 - 提供链式API来配置FastMCP认证，完全基于FastMCP的认证机制
"""

import logging
from typing import TYPE_CHECKING, Dict, Any, List, Optional

from .types import (
    AuthProviderConfig,
    AuthProviderType,
    FastMCPAuthConfig
)

if TYPE_CHECKING:
    from mcpstore.core.context import MCPStoreContext

logger = logging.getLogger(__name__)


class AuthServiceBuilder:
    """
    服务认证构建器 - 生成FastMCP认证配置
    
    这个构建器专门用于配置FastMCP的认证提供者，不实现自定义认证逻辑。
    所有认证功能都依赖FastMCP的标准认证机制。
    """
    
    def __init__(self, context: 'MCPStoreContext', service_name: str):
        self._context = context
        self._service_name = service_name
        self._required_scopes: List[str] = []
        self._auth_provider: Optional[AuthProviderConfig] = None
        
    def require_scopes(self, *scopes: str) -> 'AuthServiceBuilder':
        """要求权限范围（FastMCP scopes）"""
        self._required_scopes.extend(scopes)
        logger.debug(f"Added required scopes {list(scopes)} for service {self._service_name}")
        return self
    
    def use_bearer_auth(self, jwks_uri: str, issuer: str, audience: str, algorithm: str = "RS256") -> 'AuthServiceBuilder':
        """使用Bearer Token认证 - 配置FastMCP BearerAuthProvider"""
        self._auth_provider = AuthProviderConfig(
            provider_type=AuthProviderType.BEARER,
            jwks_uri=jwks_uri,
            issuer=issuer,
            audience=audience,
            algorithm=algorithm
        )
        logger.debug(f"Configured Bearer auth for service {self._service_name}")
        return self
    
    def use_oauth_auth(self, client_id: str, client_secret: str, base_url: str, 
                      provider: str = "custom") -> 'AuthServiceBuilder':
        """使用OAuth认证 - 配置FastMCP OAuth提供者"""
        provider_type = AuthProviderType.OAUTH
        if provider.lower() == "google":
            provider_type = AuthProviderType.GOOGLE
        elif provider.lower() == "github":
            provider_type = AuthProviderType.GITHUB
        elif provider.lower() == "workos":
            provider_type = AuthProviderType.WORKOS
        
        self._auth_provider = AuthProviderConfig(
            provider_type=provider_type,
            client_id=client_id,
            client_secret=client_secret,
            base_url=base_url
        )
        logger.debug(f"Configured {provider} OAuth auth for service {self._service_name}")
        return self
    
    def use_google_auth(self, client_id: str, client_secret: str, base_url: str,
                       required_scopes: List[str] = None) -> 'AuthServiceBuilder':
        """使用Google OAuth认证"""
        self._auth_provider = AuthProviderConfig(
            provider_type=AuthProviderType.GOOGLE,
            client_id=client_id,
            client_secret=client_secret,
            base_url=base_url,
            required_scopes=required_scopes or ["openid", "email", "profile"]
        )
        logger.debug(f"Configured Google OAuth auth for service {self._service_name}")
        return self
    
    def use_github_auth(self, client_id: str, client_secret: str, base_url: str,
                       required_scopes: List[str] = None) -> 'AuthServiceBuilder':
        """使用GitHub OAuth认证"""
        self._auth_provider = AuthProviderConfig(
            provider_type=AuthProviderType.GITHUB,
            client_id=client_id,
            client_secret=client_secret,
            base_url=base_url,
            required_scopes=required_scopes or ["read:user", "user:email"]
        )
        logger.debug(f"Configured GitHub OAuth auth for service {self._service_name}")
        return self
    
    def use_workos_auth(self, authkit_domain: str, base_url: str) -> 'AuthServiceBuilder':
        """使用WorkOS认证"""
        self._auth_provider = AuthProviderConfig(
            provider_type=AuthProviderType.WORKOS,
            base_url=base_url,
            config={"authkit_domain": authkit_domain}
        )
        logger.debug(f"Configured WorkOS auth for service {self._service_name}")
        return self
    
    def generate_fastmcp_config(self) -> Optional[FastMCPAuthConfig]:
        """生成FastMCP认证配置"""
        if not self._auth_provider:
            logger.warning(f"No auth provider configured for service {self._service_name}")
            return None
        
        return generate_fastmcp_auth_config(self._auth_provider)




def generate_fastmcp_auth_config(auth_provider: AuthProviderConfig) -> FastMCPAuthConfig:
    """根据认证提供者配置生成FastMCP认证配置"""
    
    if auth_provider.provider_type == AuthProviderType.BEARER:
        return FastMCPAuthConfig.for_bearer_token(
            jwks_uri=auth_provider.jwks_uri,
            issuer=auth_provider.issuer,
            audience=auth_provider.audience,
            algorithm=auth_provider.algorithm
        )
    
    elif auth_provider.provider_type == AuthProviderType.GOOGLE:
        return FastMCPAuthConfig.for_google_oauth(
            client_id=auth_provider.client_id,
            client_secret=auth_provider.client_secret,
            base_url=auth_provider.base_url,
            required_scopes=auth_provider.required_scopes
        )
    
    elif auth_provider.provider_type == AuthProviderType.GITHUB:
        return FastMCPAuthConfig.for_github_oauth(
            client_id=auth_provider.client_id,
            client_secret=auth_provider.client_secret,
            base_url=auth_provider.base_url,
            required_scopes=auth_provider.required_scopes
        )
    
    elif auth_provider.provider_type == AuthProviderType.WORKOS:
        return FastMCPAuthConfig.for_workos_oauth(
            authkit_domain=auth_provider.config.get("authkit_domain"),
            base_url=auth_provider.base_url
        )
    
    else:
        raise ValueError(f"Unsupported auth provider type: {auth_provider.provider_type}")
