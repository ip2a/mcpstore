"""Legacy import location for the split Python store API."""

from mcpstore.cache.proxy import RustCacheProxy
from mcpstore.native.records import RustRecordView
from mcpstore.service.proxy import RustServiceProxy
from mcpstore.sessions.session import RustSession
from mcpstore.store.store import MCPStore, RustStoreBackend
from mcpstore.context.store_context import RustStoreContext
from mcpstore.tools.proxy import RustToolProxy

__all__ = [
    "MCPStore",
    "RustCacheProxy",
    "RustRecordView",
    "RustServiceProxy",
    "RustSession",
    "RustStoreBackend",
    "RustStoreContext",
    "RustToolProxy",
]
