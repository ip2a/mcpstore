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

MCPStoreContext = RustStoreContext
ServiceProxy = RustServiceProxy
ToolProxy = RustToolProxy
CacheProxy = RustCacheProxy
Session = RustSession
SessionContext = RustSession

__all__ = [
    "MCPStore",
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
