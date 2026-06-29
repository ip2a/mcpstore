"""Rust-backed compatibility helpers for resources and prompts."""

from __future__ import annotations

from typing import Any, Dict, Optional


class ResourcesPromptsMixin:
    """Compatibility mixin delegating resources/prompts to Rust-backed context.

    Historical Python MCPStore exposed this mixin from
    ``mcpstore.core.context.resources_prompts``. The current implementation keeps
    the import and method names, but the business behavior is owned by the
    Rust-backed context object.
    """

    def list_changed_tools(
        self,
        service_name: Optional[str] = None,
        force_refresh: bool = False,
    ) -> Dict[str, Any]:
        return self._rust_context().list_changed_tools(
            service_name=service_name,
            force_refresh=force_refresh,
        )

    async def list_changed_tools_async(
        self,
        service_name: Optional[str] = None,
        force_refresh: bool = False,
    ) -> Dict[str, Any]:
        return self.list_changed_tools(service_name=service_name, force_refresh=force_refresh)

    def list_resources(self, service_name: Optional[str] = None):
        return self._rust_context().list_resources(service_name=service_name)

    async def list_resources_async(self, service_name: Optional[str] = None):
        return self.list_resources(service_name=service_name)

    def list_resource_templates(self, service_name: Optional[str] = None):
        return self._rust_context().list_resource_templates(service_name=service_name)

    async def list_resource_templates_async(self, service_name: Optional[str] = None):
        return self.list_resource_templates(service_name=service_name)

    def read_resource(self, uri: str, service_name: Optional[str] = None):
        return self._rust_context().read_resource(uri, service_name=service_name)

    async def read_resource_async(self, uri: str, service_name: Optional[str] = None):
        return self.read_resource(uri, service_name=service_name)

    def list_prompts(self, service_name: Optional[str] = None):
        return self._rust_context().list_prompts(service_name=service_name)

    async def list_prompts_async(self, service_name: Optional[str] = None):
        return self.list_prompts(service_name=service_name)

    def get_prompt(
        self,
        name: str,
        arguments: Optional[Dict[str, Any]] = None,
        service_name: Optional[str] = None,
    ):
        return self._rust_context().get_prompt(name, arguments, service_name=service_name)

    async def get_prompt_async(
        self,
        name: str,
        arguments: Optional[Dict[str, Any]] = None,
        service_name: Optional[str] = None,
    ):
        return self.get_prompt(name, arguments=arguments, service_name=service_name)

    def _rust_context(self):
        context = getattr(self, "_context", None)
        if context is not None:
            return context
        return self


__all__ = ["ResourcesPromptsMixin"]
