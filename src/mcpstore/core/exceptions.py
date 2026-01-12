"""
MCPStore Unified Exception System
Provides a comprehensive exception hierarchy for both SDK and API usage
"""

import logging
import traceback
import uuid
from datetime import datetime, UTC
from enum import Enum
from typing import Optional, Dict, Any, Union

logger = logging.getLogger(__name__)


class ErrorSeverity(Enum):
    """Error severity levels"""
    INFO = "info"
    DEGRADED = "degraded"
    ERROR = "error"
    CRITICAL = "critical"


class ErrorCode(Enum):
    """Unified error codes with HTTP status mapping"""
    
    # General errors (500)
    INTERNAL_ERROR = "INTERNAL_ERROR"
    UNKNOWN_ERROR = "UNKNOWN_ERROR"
    
    # Service errors (404, 503)
    SERVICE_NOT_FOUND = "SERVICE_NOT_FOUND"
    SERVICE_UNAVAILABLE = "SERVICE_UNAVAILABLE"
    SERVICE_CONNECTION_ERROR = "SERVICE_CONNECTION_ERROR"
    
    # Tool errors (404, 500)
    TOOL_NOT_FOUND = "TOOL_NOT_FOUND"
    TOOL_EXECUTION_ERROR = "TOOL_EXECUTION_ERROR"
    
    # Configuration errors (400)
    CONFIG_INVALID = "CONFIG_INVALID"
    CONFIG_NOT_FOUND = "CONFIG_NOT_FOUND"
    INVALID_PARAMETER = "INVALID_PARAMETER"
    INVALID_REQUEST = "INVALID_REQUEST"
    
    # Agent errors (404)
    AGENT_NOT_FOUND = "AGENT_NOT_FOUND"
    
    # Authentication/Authorization errors (401, 403)
    AUTHENTICATION_REQUIRED = "AUTHENTICATION_REQUIRED"
    AUTHORIZATION_FAILED = "AUTHORIZATION_FAILED"
    
    # Rate limiting (429)
    RATE_LIMIT_EXCEEDED = "RATE_LIMIT_EXCEEDED"
    
    def to_http_status(self) -> int:
        """Map error code to HTTP status code"""
        mapping = {
            # 400 Bad Request
            self.CONFIG_INVALID: 400,
            self.INVALID_PARAMETER: 400,
            self.INVALID_REQUEST: 400,
            
            # 401 Unauthorized
            self.AUTHENTICATION_REQUIRED: 401,
            
            # 403 Forbidden
            self.AUTHORIZATION_FAILED: 403,
            
            # 404 Not Found
            self.SERVICE_NOT_FOUND: 404,
            self.TOOL_NOT_FOUND: 404,
            self.AGENT_NOT_FOUND: 404,
            self.CONFIG_NOT_FOUND: 404,
            
            # 429 Too Many Requests
            self.RATE_LIMIT_EXCEEDED: 429,
            
            # 500 Internal Server Error
            self.INTERNAL_ERROR: 500,
            self.UNKNOWN_ERROR: 500,
            self.TOOL_EXECUTION_ERROR: 500,
            
            # 503 Service Unavailable
            self.SERVICE_UNAVAILABLE: 503,
            self.SERVICE_CONNECTION_ERROR: 503,
        }
        return mapping.get(self, 500)


class MCPStoreException(Exception):
    """Unified base exception for MCPStore
    
    This exception class is used for both SDK and API contexts.
    It provides structured error information including error codes,
    severity levels, and detailed context.
    """
    
    def __init__(
        self,
        message: str,
        error_code: Union[ErrorCode, str] = ErrorCode.INTERNAL_ERROR,
        severity: ErrorSeverity = ErrorSeverity.ERROR,
        status_code: Optional[int] = None,
        details: Optional[Dict[str, Any]] = None,
        cause: Optional[Exception] = None,
        field: Optional[str] = None,
    ):
        """Initialize MCPStore exception
        
        Args:
            message: Human-readable error message
            error_code: Error code (ErrorCode enum or string)
            severity: Error severity level
            status_code: HTTP status code (auto-derived from error_code if not provided)
            details: Additional error details
            cause: Original exception that caused this error
            field: Field name that caused the error (for validation errors)
        """
        self.message = message
        
        # Handle ErrorCode enum
        if isinstance(error_code, ErrorCode):
            self.error_code = error_code.value
            self.status_code = status_code or error_code.to_http_status()
        else:
            self.error_code = error_code
            self.status_code = status_code or 500
        
        self.severity = severity
        self.field = field
        self.details = details or {}
        self.cause = cause
        self.timestamp = datetime.now(UTC)
        self.error_id = str(uuid.uuid4())[:8]
        
        # Capture stack trace if cause is provided
        if cause:
            self.stack_trace = "".join(traceback.format_exception(type(cause), cause, cause.__traceback__))
        else:
            self.stack_trace = None
        
        super().__init__(self.message)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert exception to dictionary (for API responses)
        
        Returns:
            Dictionary representation of the exception
        """
        result = {
            "error_id": self.error_id,
            "error_code": self.error_code,
            "message": self.message,
            "severity": self.severity.value,
            "timestamp": self.timestamp.isoformat(),
        }
        
        if self.field:
            result["field"] = self.field
        
        if self.details:
            result["details"] = self.details
        
        if self.stack_trace:
            result["stack_trace"] = self.stack_trace
        
        return result
    
    def __str__(self) -> str:
        """String representation"""
        return f"[{self.error_code}] {self.message} (error_id: {self.error_id})"
    
    def __repr__(self) -> str:
        """Detailed representation"""
        return (
            f"MCPStoreException("
            f"error_code={self.error_code}, "
            f"message={self.message!r}, "
            f"error_id={self.error_id})"
        )


# === Specific Exception Classes ===

class ServiceNotFoundException(MCPStoreException):
    """Service not found exception"""
    
    def __init__(self, service_name: str, agent_id: Optional[str] = None, **kwargs):
        details = {"service_name": service_name}
        if agent_id:
            details["agent_id"] = agent_id
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=f"Service '{service_name}' not found",
            error_code=ErrorCode.SERVICE_NOT_FOUND,
            field="service_name",
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


class ServiceConnectionError(MCPStoreException):
    """Service connection error"""
    
    def __init__(self, service_name: str, reason: Optional[str] = None, **kwargs):
        message = f"Failed to connect to service '{service_name}'"
        if reason:
            message += f": {reason}"
        
        details = {"service_name": service_name}
        if reason:
            details["reason"] = reason
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=message,
            error_code=ErrorCode.SERVICE_CONNECTION_ERROR,
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


class ServiceUnavailableError(MCPStoreException):
    """Service unavailable error"""
    
    def __init__(self, service_name: str, reason: Optional[str] = None, **kwargs):
        message = f"Service '{service_name}' is unavailable"
        if reason:
            message += f": {reason}"
        
        details = {"service_name": service_name}
        if reason:
            details["reason"] = reason
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=message,
            error_code=ErrorCode.SERVICE_UNAVAILABLE,
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


class ToolNotFoundException(MCPStoreException):
    """Tool not found exception"""
    
    def __init__(self, tool_name: str, service_name: Optional[str] = None, **kwargs):
        message = f"Tool '{tool_name}' not found"
        if service_name:
            message += f" in service '{service_name}'"
        
        details = {"tool_name": tool_name}
        if service_name:
            details["service_name"] = service_name
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=message,
            error_code=ErrorCode.TOOL_NOT_FOUND,
            field="tool_name",
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


class ToolExecutionError(MCPStoreException):
    """Tool execution error"""
    
    def __init__(self, tool_name: str, reason: Optional[str] = None, **kwargs):
        message = f"Failed to execute tool '{tool_name}'"
        if reason:
            message += f": {reason}"
        
        details = {"tool_name": tool_name}
        if reason:
            details["reason"] = reason
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=message,
            error_code=ErrorCode.TOOL_EXECUTION_ERROR,
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


class ConfigurationException(MCPStoreException):
    """Configuration exception"""
    
    def __init__(self, message: str, config_path: Optional[str] = None, **kwargs):
        details = {}
        if config_path:
            details["config_path"] = config_path
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=message,
            error_code=ErrorCode.CONFIG_INVALID,
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


class ValidationException(MCPStoreException):
    """Validation exception"""
    
    def __init__(self, message: str, field: Optional[str] = None, **kwargs):
        super().__init__(
            message=message,
            error_code=ErrorCode.INVALID_PARAMETER,
            field=field,
            **kwargs
        )


class AgentNotFoundException(MCPStoreException):
    """Agent not found exception"""
    
    def __init__(self, agent_id: str, **kwargs):
        super().__init__(
            message=f"Agent '{agent_id}' not found",
            error_code=ErrorCode.AGENT_NOT_FOUND,
            field="agent_id",
            details={"agent_id": agent_id, **kwargs.get("details", {})},
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


# === Cache-Related Exceptions (for py-key-value integration) ===

class CacheOperationError(MCPStoreException):
    """Cache operation error
    
    This exception is raised when a cache operation fails.
    It is typically used to wrap py-key-value KeyValueOperationError exceptions.
    
    Validates: Requirements 6.4 (Exception and Error Handling)
    """
    
    def __init__(self, message: str, operation: Optional[str] = None, **kwargs):
        details = {}
        if operation:
            details["operation"] = operation
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=message,
            error_code=ErrorCode.INTERNAL_ERROR,
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


class CacheConnectionError(MCPStoreException):
    """Cache connection error
    
    This exception is raised when unable to connect to the cache backend.
    It is typically used to wrap py-key-value StoreConnectionError exceptions.
    
    Validates: Requirements 6.4 (Exception and Error Handling)
    """
    
    def __init__(self, message: str, backend_type: Optional[str] = None, **kwargs):
        details = {}
        if backend_type:
            details["backend_type"] = backend_type
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=message,
            error_code=ErrorCode.SERVICE_CONNECTION_ERROR,
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


class CacheValidationError(MCPStoreException):
    """Cache validation error
    
    This exception is raised when cache data validation fails.
    It is typically used to wrap py-key-value validation-related exceptions
    such as SerializationError, DeserializationError, or InvalidKeyError.
    
    Validates: Requirements 6.4 (Exception and Error Handling)
    """
    
    def __init__(self, message: str, validation_type: Optional[str] = None, **kwargs):
        details = {}
        if validation_type:
            details["validation_type"] = validation_type
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=message,
            error_code=ErrorCode.INVALID_PARAMETER,
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


class SessionSerializationError(MCPStoreException):
    """Session serialization error
    
    This exception is raised when attempting to serialize a Session object
    that contains non-serializable references (e.g., connection objects).
    
    Session objects should always remain in memory and never be serialized
    to py-key-value storage.
    
    Validates: Requirements 3.2 (Session Object Serialization Issues)
    """
    
    def __init__(self, message: str, session_info: Optional[Dict[str, Any]] = None, **kwargs):
        details = {}
        if session_info:
            details.update(session_info)
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=message,
            error_code=ErrorCode.INVALID_PARAMETER,
            severity=ErrorSeverity.ERROR,
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


# === 工具集管理相关异常 ===

class ToolSetError(MCPStoreException):
    """工具集错误基类
    
    所有工具集管理相关的异常都继承自此类
    
    Validates: Requirements 6.2 (工具调用拦截)
    """
    
    def __init__(self, message: str, **kwargs):
        super().__init__(
            message=message,
            error_code=kwargs.pop("error_code", ErrorCode.INTERNAL_ERROR),
            **kwargs
        )


class ToolNotAvailableError(ToolSetError):
    """工具不可用错误
    
    当用户尝试调用已被移除的工具时抛出此异常
    
    Validates: Requirements 6.2 (工具调用拦截)
    """
    
    def __init__(
        self,
        tool_name: str,
        service_name: Optional[str] = None,
        agent_id: Optional[str] = None,
        **kwargs
    ):
        message = f"工具 '{tool_name}' 不可用"
        if service_name:
            message += f"（服务: {service_name}）"
        message += "。使用 add_tools() 方法启用该工具。"
        
        details = {"tool_name": tool_name}
        if service_name:
            details["service_name"] = service_name
        if agent_id:
            details["agent_id"] = agent_id
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=message,
            error_code=ErrorCode.TOOL_NOT_FOUND,
            field="tool_name",
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


class CrossAgentOperationError(ToolSetError):
    """跨 Agent 操作错误
    
    当尝试使用属于其他 Agent 的 ServiceProxy 进行操作时抛出此异常
    
    Validates: Requirements 6.9 (跨 Agent 操作防护)
    """
    
    def __init__(
        self,
        current_agent_id: str,
        service_agent_id: str,
        service_name: str,
        operation: Optional[str] = None,
        **kwargs
    ):
        message = f"不允许跨 Agent 操作：服务 '{service_name}' 属于 Agent '{service_agent_id}'，"
        message += f"但当前 Agent 为 '{current_agent_id}'"
        if operation:
            message += f"（操作: {operation}）"
        
        details = {
            "current_agent_id": current_agent_id,
            "service_agent_id": service_agent_id,
            "service_name": service_name
        }
        if operation:
            details["operation"] = operation
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=message,
            error_code=ErrorCode.AUTHORIZATION_FAILED,
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


class ServiceMappingError(ToolSetError):
    """服务映射错误
    
    当服务名称映射不存在或无效时抛出此异常
    
    Validates: Requirements 6.10 (服务映射验证)
    """
    
    def __init__(
        self,
        service_name: str,
        agent_id: Optional[str] = None,
        mapping_type: Optional[str] = None,
        **kwargs
    ):
        message = f"服务映射错误：服务 '{service_name}' 的映射不存在或无效"
        if agent_id:
            message += f"（Agent: {agent_id}）"
        
        details = {"service_name": service_name}
        if agent_id:
            details["agent_id"] = agent_id
        if mapping_type:
            details["mapping_type"] = mapping_type
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=message,
            error_code=ErrorCode.SERVICE_NOT_FOUND,
            field="service_name",
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


class DataSourceNotFoundError(ToolSetError):
    """数据源不存在错误
    
    当工具集状态数据源不存在时抛出此异常
    
    Validates: Requirements 6.10 (数据源归属验证)
    """
    
    def __init__(
        self,
        agent_id: str,
        service_name: str,
        data_type: Optional[str] = None,
        **kwargs
    ):
        message = f"数据源不存在：Agent '{agent_id}' 的服务 '{service_name}'"
        if data_type:
            message += f"（数据类型: {data_type}）"
        
        details = {
            "agent_id": agent_id,
            "service_name": service_name
        }
        if data_type:
            details["data_type"] = data_type
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=message,
            error_code=ErrorCode.SERVICE_NOT_FOUND,
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )


class ServiceBindingError(ToolSetError):
    """服务绑定错误
    
    当服务不属于当前 Agent 时抛出此异常
    
    Validates: Requirements 6.7 (服务归属验证)
    """
    
    def __init__(
        self,
        service_name: str,
        agent_id: str,
        reason: Optional[str] = None,
        **kwargs
    ):
        message = f"服务绑定错误：服务 '{service_name}' 不属于 Agent '{agent_id}'"
        if reason:
            message += f"。原因: {reason}"
        
        details = {
            "service_name": service_name,
            "agent_id": agent_id
        }
        if reason:
            details["reason"] = reason
        details.update(kwargs.get("details", {}))
        
        super().__init__(
            message=message,
            error_code=ErrorCode.AUTHORIZATION_FAILED,
            field="service_name",
            details=details,
            **{k: v for k, v in kwargs.items() if k != "details"}
        )



