# MCPStore 模块化重构
# 采用 Mixin 设计模式，保持对外接口完全兼容

from .base_store import BaseMCPStore
from .setup_manager import StoreSetupManager
from .setup_mixin import SetupMixin
from .service_query import ServiceQueryMixin
from .tool_operations import ToolOperationsMixin
from .config_management import ConfigManagementMixin
from .data_space_manager import DataSpaceManagerMixin
from .api_server import APIServerMixin
from .context_factory import ContextFactoryMixin

# 使用 Mixin 模式组合所有功能
class MCPStore(
    ServiceQueryMixin,
    ToolOperationsMixin,
    ConfigManagementMixin,
    DataSpaceManagerMixin,
    APIServerMixin,
    ContextFactoryMixin,
    SetupMixin,
    BaseMCPStore  # 基础类放在最后
):
    """
    MCPStore - Intelligent Agent Tool Service Store
    Provides context switching entry points and common operations

    This class combines all functionality through Mixin pattern while maintaining
    complete backward compatibility with the original MCPStore interface.
    """

    # 继承静态方法
    setup_store = StoreSetupManager.setup_store
    _setup_with_data_space = StoreSetupManager._setup_with_data_space
    _setup_with_standalone_config = StoreSetupManager._setup_with_standalone_config

# 保持对外接口完全不变
__all__ = ['MCPStore']
