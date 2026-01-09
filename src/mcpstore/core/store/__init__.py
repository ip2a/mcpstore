# MCPStore composition and external exports (latest, single-path architecture)

from .client_manager import ClientManager
from .composed_store import MCPStore
from .setup_manager import StoreSetupManager

# Expose only authoritative setup_store entry points
MCPStore.setup_store = staticmethod(StoreSetupManager.setup_store)
MCPStore.setup_store_async = staticmethod(StoreSetupManager.setup_store_async)

__all__ = ['MCPStore', 'ClientManager']
