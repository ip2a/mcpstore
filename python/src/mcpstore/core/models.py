"""Rust-backed API response helpers.

These helpers preserve the public FastAPI example surface while the store
operations themselves remain delegated to the Rust/PyO3 core.
"""

from __future__ import annotations

import functools
import inspect
import time
from enum import Enum
from typing import Any, Callable, Dict, Optional


class ErrorCode(str, Enum):
    INVALID_REQUEST = "invalid_request"
    INVALID_PARAMETER = "invalid_parameter"
    MISSING_PARAMETER = "missing_parameter"
    SERVICE_NOT_FOUND = "service_not_found"
    SERVICE_INITIALIZATION_FAILED = "service_initialization_failed"
    SERVICE_OPERATION_FAILED = "service_operation_failed"
    CONFIG_INVALID = "config_invalid"
    CONFIG_UPDATE_FAILED = "config_update_failed"
    INTERNAL_ERROR = "internal_error"


class ResponseBuilder:
    @staticmethod
    def success(
        message: str = "success",
        data: Any = None,
        **extra: Any,
    ) -> Dict[str, Any]:
        payload: Dict[str, Any] = {
            "success": True,
            "message": message,
            "data": data,
        }
        payload.update(extra)
        return payload

    @staticmethod
    def error(
        code: ErrorCode | str = ErrorCode.INTERNAL_ERROR,
        message: str = "error",
        details: Any = None,
        field: Optional[str] = None,
        **extra: Any,
    ) -> Dict[str, Any]:
        value = code.value if isinstance(code, ErrorCode) else str(code)
        payload: Dict[str, Any] = {
            "success": False,
            "error": {
                "code": value,
                "message": message,
            },
        }
        if details is not None:
            payload["error"]["details"] = details
        if field is not None:
            payload["error"]["field"] = field
        payload.update(extra)
        return payload


def _attach_elapsed(payload: Any, elapsed_ms: float) -> Any:
    if isinstance(payload, dict):
        meta = payload.setdefault("meta", {})
        if isinstance(meta, dict):
            meta["elapsed_ms"] = round(elapsed_ms, 3)
    return payload


def timed_response(func: Callable[..., Any]) -> Callable[..., Any]:
    if inspect.iscoroutinefunction(func):

        @functools.wraps(func)
        async def async_wrapper(*args: Any, **kwargs: Any) -> Any:
            started = time.perf_counter()
            result = await func(*args, **kwargs)
            return _attach_elapsed(result, (time.perf_counter() - started) * 1000)

        return async_wrapper

    @functools.wraps(func)
    def sync_wrapper(*args: Any, **kwargs: Any) -> Any:
        started = time.perf_counter()
        result = func(*args, **kwargs)
        return _attach_elapsed(result, (time.perf_counter() - started) * 1000)

    return sync_wrapper
