"""Compatibility alias for the Rust-backed service proxy."""

from mcpstore.core.store.rust_backend import RustServiceProxy as ServiceProxy
from .service_management import UpdateServiceAuthHelper

__all__ = ["ServiceProxy", "UpdateServiceAuthHelper"]
