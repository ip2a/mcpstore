"""Rust-backed compatibility helpers for historical tool operations."""

from __future__ import annotations

from typing import Any, Dict, Optional


class ToolOperationsMixin:
    """Compatibility mixin delegating tool operations to RustStoreContext."""

    def list_tools(self, service_name: Optional[str] = None, *, filter: str = "available"):
        return self._rust_context().list_tools(service_name=service_name, filter=filter)

    async def list_tools_async(self, service_name: Optional[str] = None, *, filter: str = "available"):
        return self.list_tools(service_name=service_name, filter=filter)

    def call_tool(self, tool_name: str, args: Optional[Dict[str, Any]] = None, return_extracted: bool = False, **kwargs):
        return self._rust_context().call_tool(
            tool_name,
            args,
            return_extracted=return_extracted,
            **kwargs,
        )

    def use_tool(self, tool_name: str, args: Optional[Dict[str, Any]] = None, return_extracted: bool = False, **kwargs):
        return self.call_tool(tool_name, args, return_extracted=return_extracted, **kwargs)

    async def call_tool_async(self, tool_name: str, args: Optional[Dict[str, Any]] = None, return_extracted: bool = False, **kwargs):
        return self.call_tool(tool_name, args, return_extracted=return_extracted, **kwargs)

    async def use_tool_async(self, tool_name: str, args: Optional[Dict[str, Any]] = None, **kwargs):
        return self.use_tool(tool_name, args, **kwargs)

    def add_tools(self, service: Any, tools: Any):
        return self._rust_context().add_tools(service, tools)

    async def add_tools_async(self, service: Any, tools: Any):
        return self.add_tools(service, tools)

    def remove_tools(self, service: Any, tools: Any):
        return self._rust_context().remove_tools(service, tools)

    async def remove_tools_async(self, service: Any, tools: Any):
        return self.remove_tools(service, tools)

    def reset_tools(self, service: Any):
        return self._rust_context().reset_tools(service)

    async def reset_tools_async(self, service: Any):
        return self.reset_tools(service)

    def get_tool_set_info(self, service: Any):
        return self._rust_context().get_tool_set_info(service)

    async def get_tool_set_info_async(self, service: Any):
        return self.get_tool_set_info(service)

    def get_tool_set_summary(self):
        return self._rust_context().get_tool_set_summary()

    async def get_tool_set_summary_async(self):
        return self.get_tool_set_summary()

    def get_tools_with_stats(self):
        tools = self.list_tools(filter="all")
        return {"total_tools": len(tools), "tools": tools}

    async def get_tools_with_stats_async(self):
        return self.get_tools_with_stats()

    def get_system_stats(self):
        return self._rust_context().get_stats()

    async def get_system_stats_async(self):
        return self.get_system_stats()

    def batch_add_services(self, services):
        for service in services:
            self._rust_context().add_service(service)
        return {"success": True, "count": len(services)}

    async def batch_add_services_async(self, services):
        return self.batch_add_services(services)

    def _rust_context(self):
        return getattr(self, "_context", self)


__all__ = ["ToolOperationsMixin"]
