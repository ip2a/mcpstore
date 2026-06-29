"""Compatibility aliases for Rust-backed sessions."""

from mcpstore.core.store.rust_backend import RustSession as Session

SessionContext = Session

__all__ = ["Session", "SessionContext"]
