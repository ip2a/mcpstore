"""Compatibility helpers for service update authentication."""

from __future__ import annotations

from typing import Any, Dict, Optional

from .service_operations import ServiceOperationsMixin


class UpdateServiceAuthHelper:
    """Build a header patch and apply it through the Rust-backed context."""

    def __init__(self, context: Any, service_name: str, config: Dict[str, Any] | None = None):
        self._context = context
        self._service_name = service_name
        self._config = dict(config or {})

    async def bearer_auth(self, auth: str):
        return await self.token(auth)

    async def token(self, token: str):
        self._headers()["Authorization"] = f"Bearer {token}"
        return await self._execute_update()

    async def api_key(self, api_key: str):
        self._headers()["X-API-Key"] = api_key
        return await self._execute_update()

    async def custom_headers(self, headers: Dict[str, str]):
        self._headers().update(headers)
        return await self._execute_update()

    def _headers(self) -> Dict[str, Any]:
        headers = self._config.setdefault("headers", {})
        if not isinstance(headers, dict):
            raise ValueError("Service headers must be a dict")
        return headers

    async def _execute_update(self):
        update_async = getattr(self._context, "update_service_async", None)
        if update_async is not None:
            await update_async(self._service_name, self._config)
        else:
            self._context.update_service(self._service_name, self._config)
        return self._context


class ServiceManagementMixin(ServiceOperationsMixin):
    """Historical service-management API backed by RustStoreContext."""

    @staticmethod
    def _apply_auth_to_update_config(
        config: Optional[Dict[str, Any]] = None,
        auth: Optional[str] = None,
        token: Optional[str] = None,
        api_key: Optional[str] = None,
        headers: Optional[Dict[str, str]] = None,
    ) -> Dict[str, Any]:
        if config is not None and not isinstance(config, dict):
            raise ValueError("Service update config must be a dict")

        final_config = dict(config or {})
        final_headers = dict(final_config.get("headers") or {})
        final_headers.update(headers or {})

        bearer = token or auth
        if bearer:
            final_headers["Authorization"] = f"Bearer {bearer}"
        if api_key:
            final_headers["X-API-Key"] = api_key
        if final_headers:
            final_config["headers"] = final_headers
        return final_config

    def update_service(
        self,
        name: str,
        config: Optional[Dict[str, Any]] = None,
        auth: Optional[str] = None,
        token: Optional[str] = None,
        api_key: Optional[str] = None,
        headers: Optional[Dict[str, str]] = None,
    ):
        if config is None and not any([auth, token, api_key, headers]):
            return UpdateServiceAuthHelper(self, name, {})

        final_config = self._apply_auth_to_update_config(
            config,
            auth=auth,
            token=token,
            api_key=api_key,
            headers=headers,
        )
        ServiceOperationsMixin.update_service(self, name, final_config)
        return self

    async def update_service_async(
        self,
        name: str,
        config: Optional[Dict[str, Any]] = None,
        auth: Optional[str] = None,
        token: Optional[str] = None,
        api_key: Optional[str] = None,
        headers: Optional[Dict[str, str]] = None,
    ):
        result = self.update_service(
            name,
            config,
            auth=auth,
            token=token,
            api_key=api_key,
            headers=headers,
        )
        return result


__all__ = ["ServiceManagementMixin", "UpdateServiceAuthHelper"]
