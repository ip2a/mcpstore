from __future__ import annotations

import logging
import time
from datetime import datetime, timezone
from typing import Any, NoReturn

from fastapi import FastAPI, Request
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse

from schemas import ApiEnvelope, ApiErrorDetail, ApiMeta


logger = logging.getLogger("mcpstore.python_api")


class ApiError(Exception):
    def __init__(
        self,
        status: int,
        code: str,
        message: str,
        *,
        field: str | None = None,
        details: Any = None,
    ) -> None:
        super().__init__(message)
        self.status = status
        self.code = code
        self.message = message
        self.field = field
        self.details = details


def _meta() -> ApiMeta:
    return ApiMeta(
        timestamp=datetime.now(timezone.utc)
        .isoformat(timespec="milliseconds")
        .replace("+00:00", "Z"),
        request_id=f"req_{time.time_ns()}",
    )


def _success(message: str, data: Any) -> ApiEnvelope:
    return ApiEnvelope(success=True, message=message, data=data, meta=_meta())


def _error_response(error: ApiError) -> JSONResponse:
    payload = ApiEnvelope(
        success=False,
        message=error.message,
        errors=[
            ApiErrorDetail(
                code=error.code,
                message=error.message,
                field=error.field,
                details=error.details,
            )
        ],
        meta=_meta(),
    )
    return JSONResponse(status_code=error.status, content=payload.model_dump(mode="json"))


def _missing(field: str) -> ApiError:
    return ApiError(400, "MISSING_PARAMETER", f"缺少 {field}", field=field)


def _invalid_parameter(message: str, field: str | None = None) -> ApiError:
    return ApiError(400, "INVALID_PARAMETER", message, field=field)


def _invalid_request(message: str) -> ApiError:
    return ApiError(400, "INVALID_REQUEST", message)


def _is_service_not_found(error: Exception) -> bool:
    message = str(error).lower()
    return any(
        marker in message
        for marker in ("service not found", "servicenotfound", "服务不存在")
    )


def _raise_core_error(
    error: Exception,
    *,
    operation: bool = False,
    service_name: str | None = None,
) -> NoReturn:
    if _is_service_not_found(error):
        name = service_name or "unknown"
        raise ApiError(
            404,
            "SERVICE_NOT_FOUND",
            f"服务不存在: {name}",
            field="service_name",
            details={"service_name": name},
        ) from error
    if operation:
        raise ApiError(
            502,
            "SERVICE_OPERATION_FAILED",
            str(error),
            details={"error_type": type(error).__name__},
        ) from error
    raise ApiError(
        500,
        "INTERNAL_ERROR",
        str(error),
        details={"error_type": type(error).__name__},
    ) from error


def register_exception_handlers(api: FastAPI) -> None:
    @api.exception_handler(ApiError)
    async def handle_api_error(request: Request, error: ApiError) -> JSONResponse:
        if error.__cause__ is not None:
            logger.error(
                "API error: %s %s -> %s",
                request.method,
                request.url.path,
                error.message,
                exc_info=(
                    type(error.__cause__),
                    error.__cause__,
                    error.__cause__.__traceback__,
                ),
            )
        else:
            logger.warning(
                "API error: %s %s -> %s",
                request.method,
                request.url.path,
                error.message,
            )
        return _error_response(error)

    @api.exception_handler(RequestValidationError)
    async def handle_validation_error(
        _request: Request, error: RequestValidationError
    ) -> JSONResponse:
        first = error.errors()[0] if error.errors() else {}
        location = first.get("loc", ())
        field = str(location[-1]) if location else None
        return _error_response(
            ApiError(400, "INVALID_REQUEST", "请求格式无效", field=field)
        )

    @api.exception_handler(Exception)
    async def handle_unexpected_error(request: Request, error: Exception) -> JSONResponse:
        logger.exception(
            "Unhandled API error: %s %s", request.method, request.url.path
        )
        return _error_response(
            ApiError(
                500,
                "INTERNAL_ERROR",
                str(error),
                details={"error_type": type(error).__name__},
            )
        )
