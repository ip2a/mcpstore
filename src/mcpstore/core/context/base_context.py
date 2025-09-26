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
from ..utils.component_control import get_component_manager
from ..utils.exceptions import ServiceNotFoundError, InvalidConfigError, DeleteServiceError
from ..monitoring import MonitoringManager, NetworkEndpoint, SystemResourceInfo
from ..monitoring.analytics import get_monitoring_manager
from ..integration.openapi_integration import get_openapi_manager
from .tool_transformation import get_transformation_manager
from .agent_service_mapper import AgentServiceMapper

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
from .session_management import SessionManagementMixin
from .advanced_features import AdvancedFeaturesMixin
from .resources_prompts import ResourcesPromptsMixin
from .agent_statistics import AgentStatisticsMixin
from .service_proxy import ServiceProxy

class MCPStoreContext(
    ServiceOperationsMixin,
    ToolOperationsMixin,
    ServiceManagementMixin,
    SessionManagementMixin,
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
        
        # 🆕 初始化会话管理（来自SessionManagementMixin）
        SessionManagementMixin.__init__(self)

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
        # Per-tool overrides (e.g., flags consumed by adapters like LangChain)
        # Keyed by "{service_name}:{tool_name}" -> { flag_name: value }
        self._tool_overrides: Dict[str, Dict[str, Any]] = {}

    def for_langchain(self) -> 'LangChainAdapter':
        """Return a LangChain adapter. If a session is active (within with_session),
        return a session-aware adapter bound to that session; otherwise return the
        standard context adapter.
        """
        # Avoid top-level import cycles
        from ...adapters.langchain_adapter import LangChainAdapter, SessionAwareLangChainAdapter

        active = getattr(self, "_active_session", None)
        if active is not None and getattr(active, "is_active", False):
            # Implicit session routing: with_session scope auto-binds LangChain tools
            return SessionAwareLangChainAdapter(self, active)

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
    # 注意：复杂的认证构建器已移除，现在使用简化的 auth/headers 参数方式
    # 如需复杂认证配置，请直接使用 FastMCP 的原生API
    
    # TODO: 如果需要保留JWT相关功能，可以在后续版本中以更简单的方式实现

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

    def find_tool(self, tool_name: str) -> 'ToolProxy':
        """
        查找指定工具并返回工具代理对象
        
        在当前上下文范围内查找工具：
        - Store 上下文: 搜索全局所有服务的工具
        - Agent 上下文: 搜索该 Agent 的所有服务的工具
        
        Args:
            tool_name: 工具名称
            
        Returns:
            ToolProxy: 工具代理对象，包含该工具的所有操作方法
            
        Example:
            # Store级别使用
            weather_tool = store.for_store().find_tool('get_current_weather')
            weather_tool.tool_info()        # 获取工具详情
            weather_tool.call_tool({...})   # 调用工具
            weather_tool.usage_stats()      # 使用统计
            
            # Agent级别使用
            demo_tool = store.for_agent('demo1').find_tool('search_tool')
            demo_tool.tool_info()           # 获取工具详情
            demo_tool.test_call({...})      # 测试调用
        """
        from .tool_proxy import ToolProxy
        return ToolProxy(self, tool_name, scope='context')

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
    
    def _tool_override_key(self, service_name: str, tool_name: str) -> str:
        """Compose stable key for tool overrides."""
        service_safe = service_name or ""
        return f"{service_safe}:{tool_name}"

    def _set_tool_override(self, service_name: str, tool_name: str, flag: str, value: Any) -> None:
        """Set an override flag for a specific tool.

        Args:
            service_name: The service that provides the tool (agent-local or global depending on context view)
            tool_name: Tool name as exposed by current context's tools view
            flag: Override flag name, e.g., "return_direct"
            value: Override value
        """
        try:
            key = self._tool_override_key(service_name, tool_name)
            if key not in self._tool_overrides:
                self._tool_overrides[key] = {}
            self._tool_overrides[key][flag] = value
            logger.debug(f"[TOOL_OVERRIDE] set {flag}={value} for {key}")
        except Exception as e:
            logger.warning(f"[TOOL_OVERRIDE] failed to set override for {service_name}:{tool_name} flag={flag}: {e}")

    def _get_tool_override(self, service_name: str, tool_name: str, flag: str, default: Any = None) -> Any:
        """Get an override flag value for a tool, or default if not set."""
        try:
            key = self._tool_override_key(service_name, tool_name)
            return self._tool_overrides.get(key, {}).get(flag, default)
        except Exception:
            return default

    def _get_all_tool_overrides(self) -> Dict[str, Dict[str, Any]]:
        """Return a snapshot of all tool overrides."""
        return dict(self._tool_overrides)

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
