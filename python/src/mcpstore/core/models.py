"""Public models for the Rust-backed MCPStore Python API."""

from __future__ import annotations

import functools
import inspect
import time
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum
from math import ceil
from typing import Annotated, Any, Callable, Dict, Generic, List, Literal, Optional, Set, TypeVar, Union

from pydantic import BaseModel, ConfigDict, Field, field_validator


T = TypeVar("T")


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


class StoreScope(BaseModel):
    type: Literal["store"] = "store"

    model_config = ConfigDict(extra="forbid")


class AgentScope(BaseModel):
    type: Literal["agent"] = "agent"
    agent_id: str

    model_config = ConfigDict(extra="forbid")


ScopeRef = Annotated[Union[StoreScope, AgentScope], Field(discriminator="type")]


class ScopeDescriptor(BaseModel):
    config: Dict[str, Any] = Field(default_factory=dict)
    lifecycle: Optional[Dict[str, Any]] = None

    model_config = ConfigDict(extra="forbid")


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
    instance_id: str
    service_name: str
    scope: ScopeRef
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
    instance_id: str
    service_name: str
    scope: ScopeRef
    url: str = ""
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
    effective_config: Dict[str, Any] = Field(default_factory=dict)
    config_revision: Optional[Dict[str, int]] = None
    applied_config_revision: Optional[Dict[str, int]] = None
    restart_required: bool = False


class ToolInfo(BaseModel):
    instance_id: str
    service_name: Optional[str] = None
    scope: Optional[ScopeRef] = None
    name: str
    description: str
    input_schema: Optional[Dict[str, Any]] = None
    output_schema: Optional[Dict[str, Any]] = None


class ToolExecutionRequest(BaseModel):
    instance_id: str
    tool_name: str
    args: Dict[str, Any] = Field(default_factory=dict)
    session_id: Optional[str] = None
    timeout: Optional[float] = None
    progress_handler: Optional[Any] = None
    raise_on_error: bool = True

    model_config = ConfigDict(arbitrary_types_allowed=True)


class ListResponse(BaseModel, Generic[T]):
    success: bool
    message: Optional[str] = None
    items: List[T]
    total: int


class DataResponse(BaseModel, Generic[T]):
    success: bool
    message: Optional[str] = None
    data: T


class RegistrationResponse(BaseModel):
    success: bool
    message: str
    service_name: Optional[str] = None
    instance_id: Optional[str] = None


class ExecutionResponse(BaseModel):
    success: bool
    message: Optional[str] = None
    result: Optional[Any] = None
    error: Optional[str] = None


class ConfigResponse(BaseModel):
    success: bool
    message: str
    config: Optional[Dict[str, Any]] = None


class HealthResponse(BaseModel):
    success: bool
    status: str
    services: Optional[Dict[str, str]] = None


class ServiceInfoResponse(BaseModel):
    service: Optional[ServiceInfo] = None
    tools: List[Dict[str, Any]]
    connected: bool
    success: bool = True
    message: Optional[str] = None


class ServicesResponse(BaseModel):
    services: List[ServiceInfo]
    total_services: int
    total_tools: int
    success: bool = True
    message: Optional[str] = None


class ServiceConfig(BaseModel):
    name: str


class URLServiceConfig(ServiceConfig):
    url: str
    transport: Optional[str] = "streamable-http"
    headers: Optional[Dict[str, str]] = None


class CommandServiceConfig(ServiceConfig):
    command: str
    args: Optional[List[str]] = None
    env: Optional[Dict[str, str]] = None
    working_dir: Optional[str] = None


class MCPServerConfig(BaseModel):
    mcpServers: Dict[str, Dict[str, Any]]


ServiceConfigUnion = Union[URLServiceConfig, CommandServiceConfig, MCPServerConfig, Dict[str, Any]]


class AddServiceRequest(BaseModel):
    config: Dict[str, Any]

    model_config = ConfigDict(extra="forbid")


class UpdateServiceRequest(BaseModel):
    config: Dict[str, Any]

    model_config = ConfigDict(extra="forbid")

    @field_validator("config")
    @classmethod
    def reject_scope_configuration(cls, value: Dict[str, Any]) -> Dict[str, Any]:
        if "_mcpstore" in value:
            raise ValueError(
                "update_service only accepts base MCP fields; use scope endpoints for scope changes"
            )
        return value


class PatchServiceRequest(BaseModel):
    updates: Dict[str, Any]

    model_config = ConfigDict(extra="forbid")

    @field_validator("updates")
    @classmethod
    def reject_scope_configuration(cls, value: Dict[str, Any]) -> Dict[str, Any]:
        if "_mcpstore" in value:
            raise ValueError(
                "patch_service only accepts base MCP fields; use scope endpoints for scope changes"
            )
        return value


class AgentOnlyServiceRequest(BaseModel):
    config: Dict[str, Any]
    descriptor: ScopeDescriptor = Field(default_factory=ScopeDescriptor)

    model_config = ConfigDict(extra="forbid")

    @field_validator("config")
    @classmethod
    def reject_embedded_extension(cls, value: Dict[str, Any]) -> Dict[str, Any]:
        if "_mcpstore" in value:
            raise ValueError("agent-only config must contain base MCP fields only")
        return value


class ToolsResponse(BaseModel):
    tools: List[ToolInfo]
    total_tools: int
    success: bool = True
    message: Optional[str] = None


@dataclass
class AgentInfo:
    agent_id: str
    name: Optional[str] = None
    description: Optional[str] = None
    created_at: Optional[datetime] = None
    last_active: Optional[datetime] = None
    metadata: Optional[Dict[str, Any]] = None


@dataclass
class AgentServiceSummary:
    instance_id: str
    service_name: str
    scope: ScopeRef
    service_type: str
    status: ServiceConnectionState
    tool_count: int
    last_used: Optional[datetime] = None
    response_time: Optional[float] = None
    health_details: Optional[ServiceStateMetadata] = None


@dataclass
class AgentStatistics:
    agent_id: str
    service_count: int
    tool_count: int
    healthy_services: int
    unhealthy_services: int
    total_tool_executions: int
    is_active: bool = False
    last_activity: Optional[datetime] = None
    services: List[AgentServiceSummary] = field(default_factory=list)


@dataclass
class AgentsSummary:
    total_agents: int
    active_agents: int
    total_services: int
    total_tools: int
    store_services: int
    store_tools: int
    agents: List[AgentStatistics] = field(default_factory=list)


@dataclass
class ToolSetState:
    instance_id: str
    service_name: str
    scope: Dict[str, Any]
    available_tools: Set[str] = field(default_factory=set)
    created_at: float = field(default_factory=time.time)
    updated_at: float = field(default_factory=time.time)
    version: int = 1
    operation_history: List[Dict[str, Any]] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "instance_id": self.instance_id,
            "service_name": self.service_name,
            "scope": self.scope,
            "available_tools": list(self.available_tools),
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "version": self.version,
            "operation_history": self.operation_history[-10:],
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ToolSetState":
        return cls(
            instance_id=data["instance_id"],
            service_name=data["service_name"],
            scope=dict(data["scope"]),
            available_tools=set(data.get("available_tools", [])),
            created_at=data.get("created_at", time.time()),
            updated_at=data.get("updated_at", time.time()),
            version=data.get("version", 1),
            operation_history=data.get("operation_history", []),
        )

    def add_tools(self, tool_names: Set[str]) -> None:
        self.available_tools.update(tool_names)
        self.updated_at = time.time()
        self.version += 1
        self._record_operation("add", list(tool_names))

    def remove_tools(self, tool_names: Set[str]) -> None:
        self.available_tools.difference_update(tool_names)
        self.updated_at = time.time()
        self.version += 1
        self._record_operation("remove", list(tool_names))

    def reset(self, all_tools: Set[str]) -> None:
        self.available_tools = all_tools.copy()
        self.updated_at = time.time()
        self.version += 1
        self._record_operation("reset", [])

    def _record_operation(self, op_type: str, tools: List[str]) -> None:
        self.operation_history.append({"type": op_type, "tools": tools, "timestamp": time.time()})


@dataclass
class CallToolFailureResult:
    message: str
    cause: Optional[Any] = None
    _result: Any = field(init=False, repr=False)

    def __post_init__(self) -> None:
        from mcp import types as mcp_types

        text_block = mcp_types.TextContent(type="text", text=self.message)
        failure = mcp_types.CallToolResult(
            content=[text_block],
            structuredContent=None,
            isError=True,
        )
        setattr(failure, "structured_content", None)
        setattr(failure, "data", None)
        setattr(failure, "error", self.message)
        setattr(failure, "is_error", True)
        if self.cause is not None:
            setattr(failure, "cause", str(self.cause))
        self._result = failure

    def unwrap(self) -> Any:
        return self._result

    def __getattr__(self, item: str) -> Any:
        return getattr(self._result, item)


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
    def _generate_request_id() -> str:
        return f"req_{uuid.uuid4().hex[:16]}"

    @staticmethod
    def _get_timestamp() -> str:
        return datetime.now(timezone.utc).isoformat()

    @staticmethod
    def _create_meta(execution_time_ms: int = 0, request_id: Optional[str] = None) -> Dict[str, Any]:
        return {
            "timestamp": ResponseBuilder._get_timestamp(),
            "request_id": request_id or ResponseBuilder._generate_request_id(),
            "execution_time_ms": execution_time_ms,
            "api_version": "1.0.0",
        }

    @staticmethod
    def success(
        message: str = "success",
        data: Any = None,
        execution_time_ms: Optional[int] = None,
        request_id: Optional[str] = None,
        pagination: Optional[Dict[str, Any]] = None,
        **extra: Any,
    ) -> Dict[str, Any]:
        payload: Dict[str, Any] = {
            "success": True,
            "message": message,
            "data": data,
        }
        if execution_time_ms is not None or request_id is not None:
            payload["meta"] = ResponseBuilder._create_meta(execution_time_ms or 0, request_id)
        if pagination is not None:
            payload["pagination"] = pagination
        payload.update(extra)
        return payload

    @staticmethod
    def error(
        code: ErrorCode | str = ErrorCode.INTERNAL_ERROR,
        message: str = "error",
        details: Any = None,
        field: Optional[str] = None,
        execution_time_ms: Optional[int] = None,
        request_id: Optional[str] = None,
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
        if execution_time_ms is not None or request_id is not None:
            payload["meta"] = ResponseBuilder._create_meta(execution_time_ms or 0, request_id)
        payload.update(extra)
        return payload

    @staticmethod
    def errors(
        message: str,
        errors: List[Dict[str, Any]],
        execution_time_ms: Optional[int] = None,
        request_id: Optional[str] = None,
    ) -> Dict[str, Any]:
        payload = ResponseBuilder.error(
            code=errors[0].get("code", ErrorCode.INTERNAL_ERROR) if errors else ErrorCode.INTERNAL_ERROR,
            message=message,
            execution_time_ms=execution_time_ms,
            request_id=request_id,
        )
        payload["errors"] = errors
        return payload

    @staticmethod
    def paginated_list(
        message: str,
        items: List[Any],
        page: int,
        page_size: int,
        total: int,
        execution_time_ms: Optional[int] = None,
        request_id: Optional[str] = None,
    ) -> Dict[str, Any]:
        total_pages = ceil(total / page_size) if page_size > 0 else 0
        pagination = {
            "page": page,
            "page_size": page_size,
            "total": total,
            "total_pages": total_pages,
            "has_next": page < total_pages,
            "has_prev": page > 1,
        }
        return ResponseBuilder.success(
            message=message,
            data=items,
            execution_time_ms=execution_time_ms,
            request_id=request_id,
            pagination=pagination,
        )


class TimedResponseBuilder:
    def __init__(self):
        self.start_time: Optional[float] = None
        self.request_id = ResponseBuilder._generate_request_id()

    def __enter__(self) -> "TimedResponseBuilder":
        self.start_time = time.time()
        return self

    def __exit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        return None

    def _get_execution_time(self) -> int:
        if self.start_time is None:
            return 0
        return int((time.time() - self.start_time) * 1000)

    def success(self, message: str = "success", data: Any = None, **kwargs: Any) -> Dict[str, Any]:
        return ResponseBuilder.success(
            message=message,
            data=data,
            execution_time_ms=self._get_execution_time(),
            request_id=self.request_id,
            **kwargs,
        )

    def error(self, code: ErrorCode | str, message: str, **kwargs: Any) -> Dict[str, Any]:
        return ResponseBuilder.error(
            code=code,
            message=message,
            execution_time_ms=self._get_execution_time(),
            request_id=self.request_id,
            **kwargs,
        )

    def paginated_list(
        self,
        message: str,
        items: List[Any],
        page: int,
        page_size: int,
        total: int,
    ) -> Dict[str, Any]:
        return ResponseBuilder.paginated_list(
            message=message,
            items=items,
            page=page,
            page_size=page_size,
            total=total,
            execution_time_ms=self._get_execution_time(),
            request_id=self.request_id,
        )


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
    def __init__(self, instance_id: str, **kwargs: Any):
        details = {"instance_id": instance_id}
        details.update(kwargs.pop("details", {}) or {})
        super().__init__(
            message=f"Service instance '{instance_id}' not found",
            error_code=ErrorCode.SERVICE_NOT_FOUND,
            field="instance_id",
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


def paginated(
    default_page_size: int = 20,
    max_page_size: int = 100,
    page_param: str = "page",
    page_size_param: str = "page_size",
) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    def decorator(func: Callable[..., Any]) -> Callable[..., Any]:
        def _page_args(kwargs: Dict[str, Any]) -> tuple[int, int]:
            page = max(1, int(kwargs.get(page_param, 1)))
            page_size = max(1, min(int(kwargs.get(page_size_param, default_page_size)), max_page_size))
            kwargs[page_param] = page
            kwargs[page_size_param] = page_size
            return page, page_size

        def _wrap_result(result: Any, page: int, page_size: int) -> Any:
            if isinstance(result, dict) and result.get("success") is not None:
                return result
            if isinstance(result, tuple) and len(result) == 2:
                items, total = result
                return ResponseBuilder.paginated_list(
                    message=f"Retrieved {len(items)} items",
                    items=list(items),
                    page=page,
                    page_size=page_size,
                    total=int(total),
                )
            if isinstance(result, list):
                return ResponseBuilder.paginated_list(
                    message=f"Retrieved {len(result)} items",
                    items=result,
                    page=page,
                    page_size=page_size,
                    total=len(result),
                )
            raise ValueError(f"Paginated function must return a list or (items, total), got {type(result)}")

        if inspect.iscoroutinefunction(func):

            @functools.wraps(func)
            async def async_wrapper(*args: Any, **kwargs: Any) -> Any:
                page, page_size = _page_args(kwargs)
                return _wrap_result(await func(*args, **kwargs), page, page_size)

            return async_wrapper

        @functools.wraps(func)
        def sync_wrapper(*args: Any, **kwargs: Any) -> Any:
            page, page_size = _page_args(kwargs)
            return _wrap_result(func(*args, **kwargs), page, page_size)

        return sync_wrapper

    return decorator


def handle_errors(default_error_code: ErrorCode | str = ErrorCode.INTERNAL_ERROR) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    def decorator(func: Callable[..., Any]) -> Callable[..., Any]:
        if inspect.iscoroutinefunction(func):

            @functools.wraps(func)
            async def async_wrapper(*args: Any, **kwargs: Any) -> Any:
                try:
                    return await func(*args, **kwargs)
                except MCPStoreException as error:
                    return ResponseBuilder.error(
                        code=error.error_code,
                        message=error.message,
                        details=error.details,
                        field=error.field,
                    )
                except Exception as error:
                    return ResponseBuilder.error(code=default_error_code, message=str(error))

            return async_wrapper

        @functools.wraps(func)
        def sync_wrapper(*args: Any, **kwargs: Any) -> Any:
            try:
                return func(*args, **kwargs)
            except MCPStoreException as error:
                return ResponseBuilder.error(
                    code=error.error_code,
                    message=error.message,
                    details=error.details,
                    field=error.field,
                )
            except Exception as error:
                return ResponseBuilder.error(code=default_error_code, message=str(error))

        return sync_wrapper

    return decorator


def api_endpoint(func: Optional[Callable[..., Any]] = None, **_: Any) -> Any:
    def decorator(target: Callable[..., Any]) -> Callable[..., Any]:
        return timed_response(handle_errors()(target))

    if func is None:
        return decorator
    return decorator(func)
