"""
MCPStore Unified Exception System
Provides a comprehensive exception hierarchy for both SDK and API usage
"""

import logging
import traceback
import uuid
from datetime import datetime
from enum import Enum
from typing import Optional, Dict, Any, Union

logger = logging.getLogger(__name__)


class ErrorSeverity(Enum):
    """Error severity levels"""
    INFO = "info"
    WARNING = "warning"
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
        self.timestamp = datetime.utcnow()
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


# === Legacy Aliases (for backward compatibility during migration) ===

# SDK legacy names
MCPStoreError = MCPStoreException
ServiceNotFoundError = ServiceNotFoundException
InvalidConfigError = ConfigurationException
DeleteServiceError = ServiceUnavailableError

# API legacy names  
ServiceOperationException = ServiceUnavailableError

