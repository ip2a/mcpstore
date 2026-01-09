"""
Core Registry Module - 拆分重构后的服务注册管理模块

本模块将原来的巨大 core_registry.py 文件拆分为多个专门的管理器，
每个管理器负责特定的职责，同时保持完全的向后兼容性。

模块结构：
- main_registry: ServiceRegistry 主类（门面模式）
- base: 基础类和接口定义
- service_manager: 服务生命周期管理
- tool_manager: 工具信息处理和管理
- state_manager: 状态同步和元数据管理
- session_manager: 会话管理
- cache_manager: 缓存层管理
- persistence: JSON 持久化相关
- utils: 工具函数和辅助方法
"""

# 导出各个管理器类，供高级用户使用
from .base import (
    BaseManager,
    ServiceManagerInterface,
    ToolManagerInterface,
    StateManagerInterface,
    SessionManagerInterface,
    PersistenceManagerInterface,
    CacheManagerInterface,
    ManagerFactory,
    ManagerCoordinator
)
from .cache_manager import CacheManager
# 重新导出保持兼容性
from .main_registry import ServiceRegistry
from .mapping_manager import MappingManager
from .persistence import PersistenceManager
from .service_manager import ServiceManager
from .session_manager import SessionManager
from .state_manager import StateManager
from .tool_manager import ToolManager
# 导出工具类
from .utils import (
    JSONSchemaUtils,
    ConfigUtils,
    ServiceUtils,
    DataUtils,
    ValidationUtils,
    extract_description_from_schema,
    extract_type_from_schema
)

__all__ = [
    # 主要导出（向后兼容）
    'ServiceRegistry',

    # 管理器类
    'SessionManager',
    'StateManager',
    'ToolManager',
    'CacheManager',
    'PersistenceManager',
    'ServiceManager',
    'MappingManager',

    # 基础接口
    'BaseManager',
    'ServiceManagerInterface',
    'ToolManagerInterface',
    'StateManagerInterface',
    'SessionManagerInterface',
    'PersistenceManagerInterface',
    'CacheManagerInterface',
    'ManagerFactory',
    'ManagerCoordinator',

    # 工具类
    'JSONSchemaUtils',
    'ConfigUtils',
    'ServiceUtils',
    'DataUtils',
    'ValidationUtils',
    'extract_description_from_schema',
    'extract_type_from_schema'
]

# 模块版本和状态
__version__ = "2.0.0"
__status__ = "重构完成 - 已完成所有功能"

# 模块信息
__author__ = "Core Registry Refactoring Team"
__description__ = "拆分重构后的服务注册管理模块"
__all_managers__ = [
    'SessionManager',
    'StateManager',
    'ToolManager',
    'CacheManager',
    'PersistenceManager',
    'ServiceManager',
    'MappingManager'
]

def get_module_info():
    """获取模块信息"""
    return {
        "version": __version__,
        "status": __status__,
        "completed_managers": len(__all_managers__) + 1,  # 包括主类ServiceRegistry
        "total_managers": 8,
        "available_managers": __all_managers__ + ["ServiceRegistry"],
        "compatibility": "complete",  # 完全向后兼容
        "next_step": "重构已完成，可以安全删除原始文件"
    }