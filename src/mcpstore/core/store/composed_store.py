"""
Composed MCPStore class
Defines the final MCPStore by composing mixins and BaseMCPStore in one place
"""
from .base_store import BaseMCPStore
from .service_query import ServiceQueryMixin
from .tool_operations import ToolOperationsMixin
from .config_management import ConfigManagementMixin
from .data_space_manager import DataSpaceManagerMixin
from .api_server import APIServerMixin
from .context_factory import ContextFactoryMixin
from .setup_mixin import SetupMixin


class MCPStore(
    ServiceQueryMixin,
    ToolOperationsMixin,
    ConfigManagementMixin,
    DataSpaceManagerMixin,
    APIServerMixin,
    ContextFactoryMixin,
    SetupMixin,
    BaseMCPStore,
):
    """Final composed Store class"""
    pass

