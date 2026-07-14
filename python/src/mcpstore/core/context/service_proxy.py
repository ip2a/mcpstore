"""Compatibility alias for the Rust-backed service proxy."""

from mcpstore.core.store.rust_backend import RustServiceProxy as ServiceProxy

__all__ = ["ServiceProxy"]
