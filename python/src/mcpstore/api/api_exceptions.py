"""Exception handlers used by the FastAPI examples."""

from __future__ import annotations

from typing import Any

from mcpstore.core.models import ErrorCode, ResponseBuilder


async def validation_exception_handler(request: Any, exc: Exception):
    from fastapi.responses import JSONResponse

    return JSONResponse(
        status_code=422,
        content=ResponseBuilder.error(
            code=ErrorCode.INVALID_PARAMETER,
            message="Request validation failed",
            details=getattr(exc, "errors", lambda: str(exc))(),
        ),
    )


async def http_exception_handler(request: Any, exc: Exception):
    from fastapi.responses import JSONResponse

    status_code = getattr(exc, "status_code", 500)
    detail = getattr(exc, "detail", str(exc))
    return JSONResponse(
        status_code=status_code,
        content=ResponseBuilder.error(
            code=ErrorCode.INVALID_REQUEST,
            message="HTTP request failed",
            details=detail,
        ),
    )


async def general_exception_handler(request: Any, exc: Exception):
    from fastapi.responses import JSONResponse

    return JSONResponse(
        status_code=500,
        content=ResponseBuilder.error(
            code=ErrorCode.INTERNAL_ERROR,
            message=str(exc),
        ),
    )
