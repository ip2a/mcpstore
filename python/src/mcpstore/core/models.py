"""Rust-backed API response helpers.

These helpers preserve the public FastAPI example surface while the store
operations themselves remain delegated to the Rust/PyO3 core.
"""

from __future__ import annotations

import functools
import hashlib
import inspect
import time
import uuid
from datetime import datetime, timezone
from enum import Enum
from typing import Any, Callable, Dict, List, Optional, Union

from pydantic import BaseModel, ConfigDict, Field


class TransportType(str, Enum):
    STREAMABLE_HTTP = "streamable_http"
    STDIO = "stdio"
    STDIO_PYTHON = "stdio_python"
    STDIO_NODE = "stdio_node"
    STDIO_SHELL = "stdio_shell"


class ServiceConnectionState(str, Enum):
    INIT = "init"
    STARTUP = "startup"
    READY = "ready"
    HEALTHY = "healthy"
    DEGRADED = "degraded"
    CIRCUIT_OPEN = "circuit_open"
    HALF_OPEN = "half_open"
    DISCONNECTED = "disconnected"


class ServiceStateMetadata(BaseModel):
    consecutive_failures: int = 0
    consecutive_successes: int = 0
    last_ping_time: Optional[datetime] = None
    last_success_time: Optional[datetime] = None
    last_failure_time: Optional[datetime] = None
    response_time: Optional[float] = None
    error_message: Optional[str] = None
    failure_reason: Optional[str] = None
    reconnect_attempts: int = 0
    next_retry_time: Optional[datetime] = None
    state_entered_time: Optional[datetime] = None
    disconnect_reason: Optional[str] = None
    service_config: Dict[str, Any] = Field(default_factory=dict)
    service_name: Optional[str] = None
    agent_id: Optional[str] = None
    last_health_check: Optional[datetime] = None
    last_response_time: Optional[float] = None
    tool_sync_attempts: int = 0
    tools_confirmed_empty: bool = False
    last_tool_sync: Optional[datetime] = None
    window_error_rate: Optional[float] = None
    latency_p95: Optional[float] = None
    latency_p99: Optional[float] = None
    sample_size: Optional[int] = None
    hard_deadline: Optional[datetime] = None
    lease_deadline: Optional[datetime] = None


class ServiceInfo(BaseModel):
    url: str = ""
    name: str
    transport_type: TransportType
    status: ServiceConnectionState
    tool_count: int
    keep_alive: bool
    working_dir: Optional[str] = None
    env: Optional[Dict[str, str]] = None
    last_heartbeat: Optional[datetime] = None
    command: Optional[str] = None
    args: Optional[List[str]] = None
    package_name: Optional[str] = None
    state_metadata: Optional[ServiceStateMetadata] = None
    last_state_change: Optional[datetime] = None
    client_id: Optional[str] = None
    config: Dict[str, Any] = Field(default_factory=dict)


class ToolInfo(BaseModel):
    name: str
    tool_original_name: str
    service_original_name: str
    service_global_name: str
    service_name: str
    description: str
    client_id: Optional[str] = None
    inputSchema: Optional[Dict[str, Any]] = None


class ToolExecutionRequest(BaseModel):
    tool_name: str
    service_name: str
    args: Dict[str, Any] = Field(default_factory=dict)
    agent_id: Optional[str] = None
    client_id: Optional[str] = None
    session_id: Optional[str] = None
    timeout: Optional[float] = None
    progress_handler: Optional[Any] = None
    raise_on_error: bool = True

    model_config = ConfigDict(arbitrary_types_allowed=True)


class ErrorDetail(BaseModel):
    code: str
    message: str
    field: Optional[str] = None
    details: Optional[Dict[str, Any]] = None


class ResponseMeta(BaseModel):
    timestamp: str
    request_id: str
    execution_time_ms: int
    api_version: str = "1.0.0"


class Pagination(BaseModel):
    page: int
    page_size: int
    total: int
    total_pages: int
    has_next: bool
    has_prev: bool


class APIResponse(BaseModel):
    success: bool
    message: str
    data: Optional[Union[Dict[str, Any], List[Any]]] = None
    errors: Optional[List[ErrorDetail]] = None
    meta: Optional[ResponseMeta] = None
    pagination: Optional[Pagination] = None


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


class MCPStoreException(Exception):
    def __init__(
        self,
        message: str,
        error_code: ErrorCode | str = ErrorCode.INTERNAL_ERROR,
        status_code: Optional[int] = None,
        details: Optional[Dict[str, Any]] = None,
        field: Optional[str] = None,
    ):
        self.message = message
        self.error_code = error_code.value if isinstance(error_code, ErrorCode) else str(error_code)
        self.status_code = status_code or 500
        self.details = details or {}
        self.field = field
        self.timestamp = datetime.now(timezone.utc)
        self.error_id = str(uuid.uuid4())[:8]
        super().__init__(message)

    def to_dict(self) -> Dict[str, Any]:
        payload: Dict[str, Any] = {
            "error_id": self.error_id,
            "error_code": self.error_code,
            "message": self.message,
            "timestamp": self.timestamp.isoformat(),
        }
        if self.field:
            payload["field"] = self.field
        if self.details:
            payload["details"] = self.details
        return payload

    def __str__(self) -> str:
        return f"[{self.error_code}] {self.message} (error_id: {self.error_id})"


class ServiceNotFoundException(MCPStoreException):
    def __init__(self, service_name: str, agent_id: Optional[str] = None, **kwargs: Any):
        details = {"service_name": service_name}
        if agent_id:
            details["agent_id"] = agent_id
        details.update(kwargs.pop("details", {}) or {})
        super().__init__(
            message=f"Service '{service_name}' not found",
            error_code=ErrorCode.SERVICE_NOT_FOUND,
            field="service_name",
            details=details,
            **kwargs,
        )


class ToolExecutionError(MCPStoreException):
    def __init__(self, tool_name: str, reason: Optional[str] = None, **kwargs: Any):
        message = f"Failed to execute tool '{tool_name}'"
        if reason:
            message += f": {reason}"
        details = {"tool_name": tool_name}
        if reason:
            details["reason"] = reason
        details.update(kwargs.pop("details", {}) or {})
        super().__init__(
            message=message,
            error_code="tool_execution_error",
            details=details,
            **kwargs,
        )


class ValidationException(MCPStoreException):
    def __init__(self, message: str, field: Optional[str] = None, **kwargs: Any):
        super().__init__(
            message=message,
            error_code=ErrorCode.INVALID_PARAMETER,
            field=field,
            **kwargs,
        )


class ClientIDGenerator:
    @staticmethod
    def generate_deterministic_id(
        agent_id: str,
        service_name: str,
        service_config: Dict[str, Any],
        global_agent_store_id: str,
    ) -> str:
        config_str = str(sorted(service_config.items())) if service_config else ""
        config_hash = hashlib.md5(config_str.encode()).hexdigest()[:8]
        if agent_id == global_agent_store_id:
            return f"client_store_{service_name}_{config_hash}"
        return f"client_{agent_id}_{service_name}_{config_hash}"

    @staticmethod
    def parse_client_id(client_id: str) -> Dict[str, Optional[str]]:
        parts = client_id.split("_")
        if len(parts) >= 3 and parts[0] == "client":
            if parts[1] == "store":
                return {
                    "type": "store",
                    "agent_id": None,
                    "service_name": parts[2],
                    "config_hash": parts[3] if len(parts) > 3 else "",
                }
            return {
                "type": "agent",
                "agent_id": parts[1],
                "service_name": parts[2],
                "config_hash": parts[3] if len(parts) > 3 else "",
            }
        return {
            "type": "unknown",
            "agent_id": None,
            "service_name": None,
            "config_hash": None,
        }

    @staticmethod
    def is_deterministic_format(client_id: str) -> bool:
        return ClientIDGenerator.parse_client_id(client_id)["type"] in {"store", "agent"}


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
