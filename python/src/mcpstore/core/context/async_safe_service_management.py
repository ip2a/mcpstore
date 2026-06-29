"""Compatibility aliases for the removed Python async-safe service manager."""

from .service_operations import ServiceOperationsMixin as AsyncSafeServiceManagement


class AsyncSafeServiceManagementFactory:
    @staticmethod
    def create_service_management(*args, **kwargs):
        raise NotImplementedError(
            "The Python async-safe service manager was replaced by the Rust-backed context. "
            "Use MCPStore.for_store() or MCPStore.for_agent() instead."
        )

    @staticmethod
    def migrate_from_standard_management(standard_management):
        raise NotImplementedError(
            "The Python service-management migration path is not part of the Rust-backed architecture."
        )


__all__ = ["AsyncSafeServiceManagement", "AsyncSafeServiceManagementFactory"]
