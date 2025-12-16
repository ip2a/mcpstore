"""
Architecture Module - Functional Core, Imperative Shell

新架构模块，提供：
1. ServiceManagementCore - 纯同步业务逻辑核心
2. ServiceManagementAsyncShell - 异步外壳
3. ServiceManagementSyncShell - 同步外壳
4. ServiceManagementFactory - 工厂类
5. ShowConfigLogicCore - show_config 纯逻辑核心
6. ShowConfigAsyncShell - show_config 异步外壳

这个模块解决了原有的同步/异步混用导致的死锁问题。
"""

from .service_management_core import ServiceManagementCore, ServiceOperationPlan, WaitOperationPlan, ServiceOperation
from .service_management_shells import ServiceManagementAsyncShell, ServiceManagementSyncShell, ServiceManagementFactory
from .show_config_core import ShowConfigLogicCore
from .show_config_shell import ShowConfigAsyncShell

__all__ = [
    "ServiceManagementCore",
    "ServiceManagementAsyncShell",
    "ServiceManagementSyncShell",
    "ServiceManagementFactory",
    "ServiceOperationPlan",
    "WaitOperationPlan",
    "ServiceOperation",
    "ShowConfigLogicCore",
    "ShowConfigAsyncShell",
]