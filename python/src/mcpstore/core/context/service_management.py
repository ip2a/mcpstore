"""Compatibility helpers for service update authentication."""

from __future__ import annotations

from typing import Any, Dict


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


__all__ = ["UpdateServiceAuthHelper"]
