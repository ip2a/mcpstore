"""Public store exports.

Python SDK only exposes the Rust-backed store facade as the authoritative
``MCPStore`` entry point.
"""

from .rust_backend import MCPStore, RustStoreBackend
from .setup_manager import StoreSetupManager

MCPStore.setup_store = staticmethod(StoreSetupManager.setup_store)
MCPStore.setup_store_async = staticmethod(StoreSetupManager.setup_store_async)
RustStoreBackend.setup_store = staticmethod(StoreSetupManager.setup_store)
RustStoreBackend.setup_store_async = staticmethod(StoreSetupManager.setup_store_async)

__all__ = ["MCPStore"]
