"""Public store exports.

Python SDK only exposes the Rust-backed store facade as the authoritative
``MCPStore`` entry point.
"""

from .rust_backend import (
    MCPStore,
    RustCacheProxy,
    RustServiceProxy,
    RustSession,
    RustStoreBackend,
    RustStoreContext,
    RustToolProxy,
)
from .client_manager import ClientManager
from .config_management import ConfigManagementMixin
from .setup_manager import StoreSetupManager

MCPStore.setup_store = staticmethod(StoreSetupManager.setup_store)
MCPStore.setup_store_async = staticmethod(StoreSetupManager.setup_store_async)
RustStoreBackend.setup_store = staticmethod(StoreSetupManager.setup_store)
RustStoreBackend.setup_store_async = staticmethod(StoreSetupManager.setup_store_async)

MCPStoreContext = RustStoreContext
ServiceProxy = RustServiceProxy
ToolProxy = RustToolProxy
CacheProxy = RustCacheProxy
Session = RustSession
SessionContext = RustSession

__all__ = [
    "MCPStore",
    "ClientManager",
    "ConfigManagementMixin",
    "RustStoreBackend",
    "RustStoreContext",
    "RustSession",
    "RustServiceProxy",
    "RustToolProxy",
    "RustCacheProxy",
    "MCPStoreContext",
    "ServiceProxy",
    "ToolProxy",
    "CacheProxy",
    "Session",
    "SessionContext",
]
