"""
MCPStore Base Context Module
Core context classes and basic functionality
"""

import logging
from enum import Enum
from pathlib import Path
from typing import Dict, List, Optional, Any, Union, TYPE_CHECKING

from mcpstore.core.models.agent import (
    AgentsSummary, AgentStatistics, AgentServiceSummary
)
from mcpstore.core.models.service import (
    ServiceInfo, ServiceConfigUnion, ServiceConnectionState
)
from mcpstore.core.models.tool import ToolExecutionRequest, ToolInfo

from ..utils.async_sync_helper import get_global_helper
# 旧的认证系统已被新的auth模块替代，保持向后兼容
# from ..auth_security import get_auth_manager
from ..cache_performance import get_performance_optimizer
from ..component_control import get_component_manager
from ..utils.exceptions import ServiceNotFoundError, InvalidConfigError, DeleteServiceError
from ..monitoring import MonitoringManager, NetworkEndpoint, SystemResourceInfo
from ..monitoring.analytics import get_monitoring_manager
from ..integration.openapi_integration import get_openapi_manager
from ..tool_transformation import get_transformation_manager
from ..agent_service_mapper import AgentServiceMapper

# Create logger instance
logger = logging.getLogger(__name__)

from .types import ContextType

if TYPE_CHECKING:
    from ...adapters.langchain_adapter import LangChainAdapter
    from ..configuration.unified_config import UnifiedConfigManager



# Import mixin classes
from .service_operations import ServiceOperationsMixin
from .tool_operations import ToolOperationsMixin
from .service_management import ServiceManagementMixin
from .advanced_features import AdvancedFeaturesMixin
from .resources_prompts import ResourcesPromptsMixin
from .agent_statistics import AgentStatisticsMixin
from .service_proxy import ServiceProxy

class MCPStoreContext(
    ServiceOperationsMixin,
    ToolOperationsMixin,
    ServiceManagementMixin,
    AdvancedFeaturesMixin,
    ResourcesPromptsMixin,
    AgentStatisticsMixin
):
    """
    MCPStore context class
    Responsible for handling specific business operations and maintaining operational context environment
    """
    def __init__(self, store: 'MCPStore', agent_id: Optional[str] = None):
        self._store = store
        self._agent_id = agent_id
        self._context_type = ContextType.STORE if agent_id is None else ContextType.AGENT

        # Async/sync compatibility helper
        self._sync_helper = get_global_helper()

        # 🔧 修复：初始化等待策略（来自ServiceOperationsMixin）
        from .service_operations import AddServiceWaitStrategy
        self.wait_strategy = AddServiceWaitStrategy()

        # New feature manager
        self._transformation_manager = get_transformation_manager()
        self._component_manager = get_component_manager()
        self._openapi_manager = get_openapi_manager()
        # 旧认证管理器已被新的auth模块替代
        # self._auth_manager = get_auth_manager()
        self._performance_optimizer = get_performance_optimizer()
        self._monitoring_manager = get_monitoring_manager()

        # Monitoring manager - use data space manager or default path
        from pathlib import Path
        if hasattr(self._store, '_data_space_manager') and self._store._data_space_manager:
            # Use data space manager path
            data_dir = self._store._data_space_manager.get_file_path("monitoring").parent
        else:
            # Use default path (backward compatibility)
            config_dir = Path(self._store.config.json_path).parent
            data_dir = config_dir / "monitoring"

        self._monitoring = MonitoringManager(
            data_dir,
            self._store.tool_record_max_file_size,
            self._store.tool_record_retention_days
        )

        # Agent service name mapper
        # 🔧 [REFACTOR] global_agent_store不使用服务映射器，因为它使用原始服务名
        if agent_id and agent_id != "global_agent_store":
            self._service_mapper = AgentServiceMapper(agent_id)
        else:
            self._service_mapper = None

        # Extension reserved
        self._metadata: Dict[str, Any] = {}
        self._config: Dict[str, Any] = {}
        self._cache: Dict[str, Any] = {}

    def for_langchain(self) -> 'LangChainAdapter':
        """Return a LangChain adapter instance for subsequent LangChain-related operations."""
        from ...adapters.langchain_adapter import LangChainAdapter
        return LangChainAdapter(self)

    def for_llamaindex(self) -> 'LlamaIndexAdapter':
        """Return a LlamaIndex adapter (FunctionTool) for MCP tools."""
        from ...adapters.llamaindex_adapter import LlamaIndexAdapter
        return LlamaIndexAdapter(self)

    def for_crewai(self) -> 'CrewAIAdapter':
        """Return a CrewAI adapter that reuses LangChain tools for compatibility."""
        from ...adapters.crewai_adapter import CrewAIAdapter
        return CrewAIAdapter(self)

    def for_langgraph(self) -> 'LangGraphAdapter':
        """Return a LangGraph adapter that reuses LangChain tools."""
        from ...adapters.langgraph_adapter import LangGraphAdapter
        return LangGraphAdapter(self)

    def for_autogen(self) -> 'AutoGenAdapter':
        """Return an AutoGen adapter that produces Python functions for registration."""
        from ...adapters.autogen_adapter import AutoGenAdapter
        return AutoGenAdapter(self)

    def for_semantic_kernel(self) -> 'SemanticKernelAdapter':
        """Return a Semantic Kernel adapter that produces native function callables."""
        from ...adapters.semantic_kernel_adapter import SemanticKernelAdapter
        return SemanticKernelAdapter(self)

    def for_openai(self) -> 'OpenAIAdapter':
        """Return an OpenAI adapter that produces OpenAI function calling format tools."""
        from ...adapters.openai_adapter import OpenAIAdapter
        return OpenAIAdapter(self)

    # === Hub 功能扩展 ===
    
    def hub_services(self) -> 'HubServicesBuilder':
        """
        创建Hub服务打包构建器
        
        将当前上下文中已缓存的服务打包为独立的Hub服务进程。
        基于现有服务数据，不进行新的服务注册。
        
        Returns:
            HubServicesBuilder: Hub服务构建器，支持链式调用
            
        Example:
            # Store级别Hub
            hub = store.for_store().hub_services()\\
                .with_name("global-hub")\\
                .with_description("全局服务Hub")\\
                .build()
            
            # Agent级别Hub  
            hub = store.for_agent("team1").hub_services()\\
                .with_name("team-hub")\\
                .filter_services(category="api")\\
                .build()
        """
        from ..hub.builder import HubServicesBuilder
        return HubServicesBuilder(self, self._context_type.value, self._agent_id)
    
    def hub_tools(self) -> 'HubToolsBuilder':
        """
        创建Hub工具打包构建器
        
        将工具级别打包为Hub服务。
        注意：此功能在当前版本中为占位实现，后期版本将提供完整功能。
        
        Returns:
            HubToolsBuilder: Hub工具构建器
            
        Raises:
            NotImplementedError: 当前版本未实现此功能
        """
        from ..hub.builder import HubToolsBuilder
        return HubToolsBuilder(self, self._context_type.value, self._agent_id)
    
    # === 认证功能扩展 ===
    
    def auth_jwt_payload(self, client_id: str) -> 'AuthTokenBuilder':
        """
        创建JWT Payload构建器
        
        用于生成FastMCP JWT token的payload配置。
        FastMCP通过JWT token中的scopes和claims来管理用户权限。
        
        Args:
            client_id: 客户端ID（用户ID）
            
        Returns:
            AuthTokenBuilder: Token构建器，支持链式调用
            
        Example:
            # 生成JWT payload
            payload = store.for_store().auth_jwt_payload("user123")\\
                .add_scopes("read", "write", "execute")\\
                .add_claim("role", "admin")\\
                .add_claim("tenant_id", "company_abc")\\
                .generate_payload()
        """
        from ..auth.builder import AuthTokenBuilder
        return AuthTokenBuilder(self, client_id)
    
    def auth_service(self, service_name: str) -> 'AuthServiceBuilder':
        """
        创建服务认证构建器
        
        配置服务的认证保护，生成FastMCP认证配置。
        不实现实际认证逻辑，仅封装配置生成。
        
        Args:
            service_name: 服务名称
            
        Returns:
            AuthServiceBuilder: 服务认证构建器，支持链式调用
            
        Example:
            # 保护服务
            service_config = store.for_store().auth_service("payment-api")\\
                .require_scopes("payment:read", "payment:write")\\
                .set_access("admin")\\
                .use_bearer_auth(
                    jwks_uri="https://auth.company.com/.well-known/jwks.json",
                    issuer="https://auth.company.com",
                    audience="payment-service"
                )\\
                .protect()
        """
        from ..auth.builder import AuthServiceBuilder
        return AuthServiceBuilder(self, service_name)
    
    def auth_provider(self, provider_type: str) -> 'AuthProviderBuilder':
        """
        创建认证提供者构建器
        
        配置认证提供者，生成FastMCP认证提供者配置。
        支持bearer、oauth、google、github、workos等类型。
        
        Args:
            provider_type: 认证提供者类型 (bearer, oauth, google, github, workos)
            
        Returns:
            AuthProviderBuilder: 认证提供者构建器，支持链式调用
            
        Example:
            # 配置Google OAuth
            provider_config = store.for_store().auth_provider("google")\\
                .set_client_credentials("google_client_id", "google_secret")\\
                .set_base_url("https://myserver.com")\\
                .setup()
        """
        from ..auth.builder import AuthProviderBuilder
        return AuthProviderBuilder(self, provider_type)
    
    def auth_token(self, client_id: str) -> 'AuthTokenBuilder':
        """
        创建Token构建器（用于JWT payload生成）
        
        用于生成FastMCP JWT token的payload配置。
        
        Args:
            client_id: 客户端ID
            
        Returns:
            AuthTokenBuilder: Token构建器，支持链式调用
            
        Example:
            # 生成JWT payload
            payload = store.for_store().auth_token("user123")\\
                .add_scopes("read", "write")\\
                .add_claim("role", "admin")\\
                .generate_payload()
        """
        from ..auth.builder import AuthTokenBuilder
        return AuthTokenBuilder(self, client_id)

    def find_service(self, service_name: str) -> 'ServiceProxy':
        """
        查找指定服务并返回服务代理对象
        
        进一步缩小作用域到具体服务，提供该服务的所有操作方法。
        
        Args:
            service_name: 服务名称
            
        Returns:
            ServiceProxy: 服务代理对象，包含该服务的所有操作方法
            
        Example:
            # Store级别使用
            weather_service = store.for_store().find_service('weather')
            weather_service.service_info()      # 获取服务详情
            weather_service.list_tools()       # 列出工具
            weather_service.check_health()     # 检查健康状态
            
            # Agent级别使用
            demo_service = store.for_agent('demo1').find_service('service1')
            demo_service.service_info()        # 获取服务详情
            demo_service.restart_service()     # 重启服务
        """
        from .service_proxy import ServiceProxy
        return ServiceProxy(self, service_name)

    @property
    def context_type(self) -> ContextType:
        """Get context type"""
        return self._context_type

    @property
    def agent_id(self) -> Optional[str]:
        """Get current agent_id"""
        return self._agent_id

    def get_unified_config(self) -> 'UnifiedConfigManager':
        """Get unified configuration manager

        Returns:
            UnifiedConfigManager: Unified configuration manager instance
        """
        return self._store._unified_config

    # === Monitoring and statistics functionality ===

    async def check_network_endpoints(self, endpoints: List[Dict[str, str]]) -> List[NetworkEndpoint]:
        """Check network endpoint status"""
        return await self._monitoring.check_network_endpoints(endpoints)

    def get_system_resource_info(self) -> SystemResourceInfo:
        """Get system resource information"""
        return self._monitoring.get_system_resource_info()

    async def get_system_resource_info_async(self) -> SystemResourceInfo:
        """Asynchronously get system resource information"""
        return self.get_system_resource_info()

    def record_api_call(self, response_time: float):
        """Record API call"""
        self._monitoring.record_api_call(response_time)

    def increment_active_connections(self):
        """Increment active connection count"""
        self._monitoring.increment_active_connections()

    def decrement_active_connections(self):
        """Decrement active connection count"""
        self._monitoring.decrement_active_connections()

    def get_tool_records(self, limit: int = 50) -> Dict[str, Any]:
        """Get tool execution records"""
        return self._monitoring.get_tool_records(limit)

    async def get_tool_records_async(self, limit: int = 50) -> Dict[str, Any]:
        """Asynchronously get tool execution records"""
        return self.get_tool_records(limit)

    # === Internal helper methods ===
    
    def _get_available_services(self) -> List[str]:
        """Get available service list"""
        try:
            if self._context_type == ContextType.STORE:
                services = self._store.for_store().list_services()
            else:
                services = self._store.for_agent(self._agent_id).list_services()
            return [service.name for service in services]
        except Exception:
            return []

    def _extract_original_tool_name(self, display_name: str, service_name: str) -> str:
        """
        Extract original tool name from display name

        Args:
            display_name: Display name (e.g., "weather-api_get_weather")
            service_name: Service name (e.g., "weather-api")

        Returns:
            str: Original tool name (e.g., "get_weather")
        """
        # Remove service name prefix
        if display_name.startswith(f"{service_name}_"):
            return display_name[len(service_name) + 1:]
        elif display_name.startswith(f"{service_name}__"):
            return display_name[len(service_name) + 2:]
        else:
            return display_name

    def _cleanup_reconnection_queue_for_client(self, client_id: str):
        """Clean up reconnection queue entries related to specified client"""
        try:
            # Find all reconnection entries related to this client
            if hasattr(self._store.orchestrator, 'smart_reconnection') and self._store.orchestrator.smart_reconnection:
                reconnection_manager = self._store.orchestrator.smart_reconnection

                # Get all reconnection entries
                all_entries = reconnection_manager.entries.copy()

                # Find entries to be cleaned up
                entries_to_remove = []
                for service_key, entry in all_entries.items():
                    if entry.client_id == client_id:
                        entries_to_remove.append(service_key)
                
                # Remove entries
                for service_key in entries_to_remove:
                    reconnection_manager.remove_service(service_key)
                    logger.debug(f"Removed reconnection entry for {service_key}")
                    
        except Exception as e:
            logger.warning(f"Failed to cleanup reconnection queue for client {client_id}: {e}")

    def _create_validation_function(self, rule: Dict[str, Any]) -> callable:
        """Create validation function"""
        def validate(value):
            if "min_length" in rule and len(str(value)) < rule["min_length"]:
                raise ValueError(f"Value too short, minimum length: {rule['min_length']}")
            if "max_length" in rule and len(str(value)) > rule["max_length"]:
                raise ValueError(f"Value too long, maximum length: {rule['max_length']}")
            if "pattern" in rule:
                import re
                if not re.match(rule["pattern"], str(value)):
                    raise ValueError(f"Value doesn't match pattern: {rule['pattern']}")
        return validate

    def _extract_service_name(self, tool_name: str) -> str:
        """Extract service name from tool name"""
        if "_" in tool_name:
            return tool_name.split("_")[0]
        elif "__" in tool_name:
            return tool_name.split("__")[0]
        else:
            return ""
