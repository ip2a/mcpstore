from __future__ import annotations

from typing import Any

from pydantic import BaseModel


class ApiMeta(BaseModel):
    timestamp: str
    request_id: str
    execution_time_ms: int = 0
    api_version: str = "1.0.0"


class ApiErrorDetail(BaseModel):
    code: str
    message: str
    field: str | None = None
    details: Any = None


class ApiEnvelope(BaseModel):
    success: bool
    message: str
    data: Any = None
    errors: list[ApiErrorDetail] | None = None
    meta: ApiMeta
    pagination: Any = None
