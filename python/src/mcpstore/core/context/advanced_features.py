"""Rust-backed compatibility helpers for historical advanced features."""

from __future__ import annotations


class AdvancedFeaturesMixin:
    """Compatibility mixin for advanced context methods."""

    def import_api(
        self,
        api_url: str,
        api_name: str = None,
        *,
        headers: dict = None,
        auth: dict = None,
        ref_cache: dict = None,
    ):
        import time

        name = api_name or f"api_{int(time.time())}"
        self._rust_context().import_openapi_service(
            name,
            api_url,
            headers=headers,
            auth=auth,
            ref_cache=ref_cache,
        )
        return self

    async def import_api_async(
        self,
        api_url: str,
        api_name: str = None,
        *,
        headers: dict = None,
        auth: dict = None,
        ref_cache: dict = None,
    ):
        return self.import_api(api_url, api_name, headers=headers, auth=auth, ref_cache=ref_cache)

    def import_api_from_spec(
        self,
        spec: dict,
        api_name: str,
        spec_url: str = "memory://openapi",
        *,
        headers: dict = None,
        auth: dict = None,
        ref_cache: dict = None,
    ):
        self._rust_context().import_openapi_service_from_spec(
            api_name,
            spec_url,
            spec,
            headers=headers,
            auth=auth,
            ref_cache=ref_cache,
        )
        return self

    def last_openapi_import(self):
        return self._rust_context().last_openapi_import()

    def reset_mcp_json_file(self) -> bool:
        return self._rust_context().reset_mcp_json_scope("all")

    async def reset_mcp_json_file_async(self, scope: str = "all") -> bool:
        return self._rust_context().reset_mcp_json_scope(scope)

    def _rust_context(self):
        return getattr(self, "_context", self)


__all__ = ["AdvancedFeaturesMixin"]
