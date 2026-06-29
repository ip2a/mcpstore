"""Compatibility alias for the Rust-backed cache proxy."""

from mcpstore.core.store.rust_backend import RustCacheProxy as CacheProxy

__all__ = ["CacheProxy"]
