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

from ..async_sync_helper import get_global_helper
from ..auth_security import get_auth_manager
from ..cache_performance import get_performance_optimizer
from ..component_control import get_component_manager
from ..exceptions import ServiceNotFoundError, InvalidConfigError, DeleteServiceError
from ..monitoring import MonitoringManager, NetworkEndpoint, SystemResourceInfo
from ..monitoring.analytics import get_monitoring_manager
from ..openapi_integration import get_openapi_manager
from ..tool_transformation import get_transformation_manager
from ..agent_service_mapper import AgentServiceMapper

# Create logger instance
logger = logging.getLogger(__name__)

from .types import ContextType

if TYPE_CHECKING:
    from ...adapters.langchain_adapter import LangChainAdapter
    from ..unified_config import UnifiedConfigManager



# Import mixin classes
from .service_operations import ServiceOperationsMixin
from .tool_operations import ToolOperationsMixin
from .service_management import ServiceManagementMixin
from .advanced_features import AdvancedFeaturesMixin
from .resources_prompts import ResourcesPromptsMixin
from .agent_statistics import AgentStatisticsMixin

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
        self._auth_manager = get_auth_manager()
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
