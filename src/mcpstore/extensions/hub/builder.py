"""
Hub Builder Module
Hub构建器模块 - 提供链式API构建Hub服务包
"""

import logging
from typing import TYPE_CHECKING, List, Dict, Any, Optional

from .package import HubPackage
from .types import HubConfig, HubServiceInfo

if TYPE_CHECKING:
    from mcpstore.core.context.base_context import MCPStoreContext

logger = logging.getLogger(__name__)


class HubServicesBuilder:
    """
    Hub服务打包构建器
    
    将MCPStore中已缓存的服务集合打包成独立的Hub服务进程。
    基于现有的服务缓存，不进行新的服务注册。
    
    特点：
    - 链式API设计，提供优雅的用户体验
    - 基于MCPStoreContext的现有服务数据
    - 支持服务过滤和配置定制
    - 生成可独立运行的Hub服务进程
    """
    
    def __init__(self, context: 'MCPStoreContext', context_type: str, target_id: Optional[str] = None):
        """
        初始化Hub服务构建器
        
        Args:
            context: MCPStoreContext实例，提供服务数据访问
            context_type: 上下文类型 "store" 或 "agent"
            target_id: 目标ID（仅当context_type="agent"时使用）
        """
        self._context = context
        self._context_type = context_type
        self._target_id = target_id
        
        # Hub配置
        self._config = HubConfig(
            name="default",
            context_type=context_type,
            target_id=target_id
        )
        
        logger.debug(f"HubServicesBuilder initialized for {context_type}" + 
                    (f" with target_id={target_id}" if target_id else ""))
    
    def with_name(self, name: str) -> 'HubServicesBuilder':
        """
        设置Hub服务名称
        
        Args:
            name: Hub服务名称
            
        Returns:
            HubServicesBuilder: 支持链式调用
        """
        self._config.name = name
        logger.debug(f"Hub name set to: {name}")
        return self
    
    def with_description(self, description: str) -> 'HubServicesBuilder':
        """
        设置Hub服务描述
        
        Args:
            description: Hub服务描述
            
        Returns:
            HubServicesBuilder: 支持链式调用
        """
        self._config.description = description
        logger.debug(f"Hub description set to: {description}")
        return self
    
    def enable_basic_routing(self, enabled: bool = True) -> 'HubServicesBuilder':
        """
        启用或禁用基础路由功能
        
        Args:
            enabled: 是否启用基础路由
            
        Returns:
            HubServicesBuilder: 支持链式调用
        """
        self._config.basic_routing = enabled
        logger.debug(f"Hub basic routing set to: {enabled}")
        return self
    
    def enable_auth(self, provider_type: str = "bearer") -> 'HubServicesBuilder':
        """
        启用Hub服务认证
        
        Args:
            provider_type: 认证提供者类型 (bearer, oauth, google, github, workos)
            
        Returns:
            HubServicesBuilder: 支持链式调用
        """
        self._config.auth_enabled = True
        self._config.auth_provider_type = provider_type
        logger.debug(f"Hub auth enabled with provider: {provider_type}")
        return self
    
    def set_jwt_config(self, jwks_uri: str, issuer: str, audience: str, algorithm: str = "RS256") -> 'HubServicesBuilder':
        """
        设置JWT认证配置 (for Bearer Token)
        
        Args:
            jwks_uri: JWKS URI
            issuer: JWT Issuer
            audience: JWT Audience
            algorithm: JWT算法 (默认RS256)
            
        Returns:
            HubServicesBuilder: 支持链式调用
        """
        self._config.fastmcp_auth = {
            "type": "BearerAuthProvider",
            "jwks_uri": jwks_uri,
            "issuer": issuer,
            "audience": audience,
            "algorithm": algorithm
        }
        logger.debug(f"Hub JWT config set: issuer={issuer}, audience={audience}")
        return self
    
    def set_oauth_config(self, client_id: str, client_secret: str, base_url: str, 
                        provider: str = "custom") -> 'HubServicesBuilder':
        """
        设置OAuth认证配置
        
        Args:
            client_id: OAuth客户端ID
            client_secret: OAuth客户端密钥
            base_url: 服务器基础URL
            provider: OAuth提供者 (google, github, workos, custom)
            
        Returns:
            HubServicesBuilder: 支持链式调用
        """
        provider_map = {
            "google": "GoogleProvider",
            "github": "GitHubProvider", 
            "workos": "AuthKitProvider",
            "custom": "OAuthProvider"
        }
        
        self._config.fastmcp_auth = {
            "type": provider_map.get(provider, "OAuthProvider"),
            "client_id": client_id,
            "client_secret": client_secret,
            "base_url": base_url,
            "provider": provider
        }
        logger.debug(f"Hub OAuth config set: provider={provider}, base_url={base_url}")
        return self
    
    def require_scopes(self, *scopes: str) -> 'HubServicesBuilder':
        """
        设置Hub必需的权限范围
        
        Args:
            scopes: 权限范围列表
            
        Returns:
            HubServicesBuilder: 支持链式调用
        """
        self._config.required_scopes = list(scopes)
        logger.debug(f"Hub required scopes set: {list(scopes)}")
        return self
    
    def with_port(self, port: int) -> 'HubServicesBuilder':
        """
        设置Hub服务端口
        
        Args:
            port: 端口号
            
        Returns:
            HubServicesBuilder: 支持链式调用
        """
        self._config.port = port
        logger.debug(f"Hub port set to: {port}")
        return self
    
    def filter_services(self, **filters) -> 'HubServicesBuilder':
        """
        设置服务过滤器
        
        支持的过滤器：
        - category: 服务分类
        - status: 服务状态
        - transport_type: 传输类型
        
        Args:
            **filters: 过滤条件
            
        Returns:
            HubServicesBuilder: 支持链式调用
        """
        self._config.filters.update(filters)
        logger.debug(f"Hub service filters updated: {self._config.filters}")
        return self
    
    async def build_async(self) -> HubPackage:
        """
        异步构建Hub服务包
        
        从MCPStoreContext获取已缓存的服务信息，生成Hub服务包。
        不进行新的服务注册，完全基于现有缓存数据。
        
        Returns:
            HubPackage: 可启动的Hub服务包
        """
        try:
            logger.info(f"Building Hub package '{self._config.name}' for {self._context_type}")
            
            # 1. 从Context获取服务列表（使用现有的list_services_async方法）
            services_info = await self._context.list_services_async()
            logger.debug(f"Retrieved {len(services_info)} services from context")
            
            # 2. 转换为Hub服务信息格式
            hub_services = self._convert_to_hub_services(services_info)
            
            # 3. 应用过滤器
            if self._config.filters:
                hub_services = self._apply_filters(hub_services, self._config.filters)
                logger.debug(f"After filtering: {len(hub_services)} services")
            
            # 4. 生成包名
            package_name = self._generate_package_name()
            
            # 5. 创建Hub包
            hub_package = HubPackage(
                package_name=package_name,
                services=hub_services,
                config=self._config
            )
            
            logger.info(f"Hub package '{package_name}' built successfully with {len(hub_services)} services")
            return hub_package
            
        except Exception as e:
            logger.error(f"Failed to build Hub package: {e}")
            raise
    
    def build(self) -> HubPackage:
        """
        同步构建Hub服务包
        
        这是build_async的同步版本，使用MCPStore的异步同步助手。
        
        Returns:
            HubPackage: 可启动的Hub服务包
        """
        # 使用Context的同步助手运行异步方法
        return self._context._sync_helper.run_async(self.build_async())
    
    def _convert_to_hub_services(self, services_info: List) -> List[HubServiceInfo]:
        """
        将MCPStore的ServiceInfo转换为Hub的HubServiceInfo
        
        Args:
            services_info: MCPStore的服务信息列表
            
        Returns:
            List[HubServiceInfo]: Hub服务信息列表
        """
        hub_services = []
        
        for service in services_info:
            # 提取工具信息
            tools = []
            if hasattr(service, 'tools') and service.tools:
                tools = [
                    {
                        "name": tool.get("name", "unknown"),
                        "description": tool.get("description", ""),
                        "parameters": tool.get("parameters", {})
                    }
                    for tool in service.tools
                ]
            
            # 创建Hub服务信息
            hub_service = HubServiceInfo(
                name=service.name,
                url=getattr(service, 'url', None),
                command=getattr(service, 'command', None),
                args=getattr(service, 'args', []),
                transport_type=getattr(service, 'transport_type', 'unknown'),
                status=getattr(service, 'status', 'unknown'),
                tools=tools
            )
            
            hub_services.append(hub_service)
            
        logger.debug(f"Converted {len(hub_services)} services to Hub format")
        return hub_services
    
    def _apply_filters(self, services: List[HubServiceInfo], filters: Dict[str, Any]) -> List[HubServiceInfo]:
        """
        应用服务过滤器
        
        Args:
            services: 服务列表
            filters: 过滤条件
            
        Returns:
            List[HubServiceInfo]: 过滤后的服务列表
        """
        filtered = services
        
        # 按分类过滤
        if 'category' in filters:
            category = filters['category']
            # 注意：这里需要根据实际的ServiceInfo结构调整
            filtered = [s for s in filtered if getattr(s, 'category', None) == category]
        
        # 按状态过滤
        if 'status' in filters:
            status = filters['status']
            filtered = [s for s in filtered if s.status == status]
        
        # 按传输类型过滤
        if 'transport_type' in filters:
            transport_type = filters['transport_type']
            filtered = [s for s in filtered if s.transport_type == transport_type]
        
        # 按服务名称模式过滤
        if 'name_pattern' in filters:
            pattern = filters['name_pattern']
            import re
            filtered = [s for s in filtered if re.search(pattern, s.name, re.IGNORECASE)]
        
        logger.debug(f"Applied filters {filters}, {len(services)} -> {len(filtered)} services")
        return filtered
    
    def _generate_package_name(self) -> str:
        """
        生成Hub包名
        
        Returns:
            str: Hub包名
        """
        if self._context_type == "store":
            return f"store-hub-{self._config.name}"
        else:
            return f"agent-{self._target_id}-hub-{self._config.name}"


class HubToolsBuilder:
    """
    Hub工具打包构建器
    
    后期实现：将工具级别打包为Hub服务。
    当前版本仅提供接口占位，实际功能在后续版本中实现。
    """
    
    def __init__(self, context: 'MCPStoreContext', context_type: str, target_id: Optional[str] = None):
        """
        初始化Hub工具构建器
        
        Args:
            context: MCPStoreContext实例
            context_type: 上下文类型
            target_id: 目标ID
        """
        self._context = context
        self._context_type = context_type
        self._target_id = target_id
        
        logger.debug("HubToolsBuilder initialized (placeholder implementation)")
    
    async def build_async(self):
        """异步构建Hub工具包 - 后期实现"""
        raise NotImplementedError("Hub tools功能将在后期版本实现")
    
    def build(self):
        """同步构建Hub工具包 - 后期实现"""
        raise NotImplementedError("Hub tools功能将在后期版本实现")
