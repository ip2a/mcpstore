"""
标准错误码定义（增强版）

特性：
- 使用Enum提供类型安全
- 支持HTTP状态码映射
- 支持错误描述（国际化准备）
- 分类管理

创建日期: 2025-10-01
"""

from enum import Enum
from typing import Dict


class ErrorCode(str, Enum):
    """标准错误码枚举（增强版）
    
    分类：
    - 1xxx: 通用错误
    - 2xxx: 服务相关
    - 3xxx: 工具相关
    - 4xxx: Agent相关
    - 5xxx: 配置相关
    - 6xxx: 认证相关
    
    使用示例：
        from mcpstore.core.models.error_codes import ErrorCode
        
        # 使用错误码
        code = ErrorCode.SERVICE_NOT_FOUND
        
        # 获取HTTP状态码
        status = code.to_http_status()  # 404
        
        # 获取错误描述
        desc = code.get_description()  # "The requested service does not exist"
    """
    
    # ==================== 通用错误 (1xxx) ====================
    
    INTERNAL_ERROR = "INTERNAL_ERROR"
    """服务器内部错误。意外的异常或系统故障"""
    
    INVALID_PARAMETER = "INVALID_PARAMETER"
    """参数无效。参数格式错误、类型错误或不符合要求"""
    
    MISSING_PARAMETER = "MISSING_PARAMETER"
    """缺少必需参数。必填字段未提供"""
    
    INVALID_REQUEST = "INVALID_REQUEST"
    """请求无效。请求格式错误或不符合API规范"""
    
    OPERATION_TIMEOUT = "OPERATION_TIMEOUT"
    """操作超时。操作执行时间超过限制"""
    
    RATE_LIMIT_EXCEEDED = "RATE_LIMIT_EXCEEDED"
    """速率限制超出。请求频率超过限制"""
    
    # ==================== 服务相关 (2xxx) ====================
    
    SERVICE_NOT_FOUND = "SERVICE_NOT_FOUND"
    """服务未找到。指定的服务名称不存在"""
    
    SERVICE_ALREADY_EXISTS = "SERVICE_ALREADY_EXISTS"
    """服务已存在。尝试添加重复的服务"""
    
    SERVICE_UNAVAILABLE = "SERVICE_UNAVAILABLE"
    """服务不可用。服务处于不可用状态（disconnected/unreachable）"""
    
    SERVICE_TIMEOUT = "SERVICE_TIMEOUT"
    """服务超时。连接或操作服务时超时"""
    
    SERVICE_CONNECTION_FAILED = "SERVICE_CONNECTION_FAILED"
    """服务连接失败。无法建立与服务的连接"""
    
    SERVICE_INITIALIZATION_FAILED = "SERVICE_INITIALIZATION_FAILED"
    """服务初始化失败。服务启动或初始化过程出错"""
    
    SERVICE_CONFIGURATION_INVALID = "SERVICE_CONFIGURATION_INVALID"
    """服务配置无效。配置参数不正确或缺失"""
    
    # ==================== 工具相关 (3xxx) ====================
    
    TOOL_NOT_FOUND = "TOOL_NOT_FOUND"
    """工具未找到。指定的工具名称不存在"""
    
    TOOL_EXECUTION_FAILED = "TOOL_EXECUTION_FAILED"
    """工具执行失败。工具运行时发生错误"""
    
    TOOL_PARAMETER_INVALID = "TOOL_PARAMETER_INVALID"
    """工具参数无效。提供的参数不符合工具要求"""
    
    TOOL_TIMEOUT = "TOOL_TIMEOUT"
    """工具执行超时。工具执行时间超过限制"""
    
    TOOL_UNAVAILABLE = "TOOL_UNAVAILABLE"
    """工具不可用。工具所属服务不可用或工具被禁用"""
    
    # ==================== Agent相关 (4xxx) ====================
    
    AGENT_NOT_FOUND = "AGENT_NOT_FOUND"
    """Agent未找到。指定的Agent ID不存在"""
    
    AGENT_ALREADY_EXISTS = "AGENT_ALREADY_EXISTS"
    """Agent已存在。尝试创建重复的Agent"""
    
    AGENT_OPERATION_FAILED = "AGENT_OPERATION_FAILED"
    """Agent操作失败。Agent级别操作执行失败"""
    
    # ==================== 配置相关 (5xxx) ====================
    
    CONFIG_NOT_FOUND = "CONFIG_NOT_FOUND"
    """配置未找到。指定的配置项不存在"""
    
    CONFIG_INVALID = "CONFIG_INVALID"
    """配置无效。配置格式或内容不正确"""
    
    CONFIG_UPDATE_FAILED = "CONFIG_UPDATE_FAILED"
    """配置更新失败。更新配置时发生错误"""
    
    # ==================== 认证相关 (6xxx) ====================
    
    AUTHENTICATION_REQUIRED = "AUTHENTICATION_REQUIRED"
    """需要认证。访问受保护资源但未提供认证信息"""
    
    AUTHENTICATION_FAILED = "AUTHENTICATION_FAILED"
    """认证失败。提供的认证信息无效"""
    
    AUTHORIZATION_FAILED = "AUTHORIZATION_FAILED"
    """授权失败。认证成功但无权限执行操作"""
    
    TOKEN_EXPIRED = "TOKEN_EXPIRED"
    """令牌过期。认证令牌已过期需要刷新"""
    
    TOKEN_INVALID = "TOKEN_INVALID"
    """令牌无效。提供的令牌格式错误或被篡改"""
    
    # ==================== 增强方法 ====================
    
    def to_http_status(self) -> int:
        """映射到HTTP状态码
        
        Returns:
            int: HTTP状态码（如404, 500等）
            
        Example:
            >>> ErrorCode.SERVICE_NOT_FOUND.to_http_status()
            404
        """
        return _ERROR_CODE_TO_HTTP_STATUS.get(self, 500)
    
    def get_description(self) -> str:
        """获取错误描述（英文）
        
        Returns:
            str: 错误的详细描述
            
        Example:
            >>> ErrorCode.SERVICE_NOT_FOUND.get_description()
            'The requested service does not exist'
        """
        return _ERROR_CODE_DESCRIPTIONS.get(self, "An error occurred")
    
    def get_category(self) -> str:
        """获取错误分类
        
        Returns:
            str: 错误分类名称
            
        Example:
            >>> ErrorCode.SERVICE_NOT_FOUND.get_category()
            'Service'
        """
        return _ERROR_CODE_CATEGORIES.get(self, "Unknown")


# ==================== 映射表 ====================

_ERROR_CODE_TO_HTTP_STATUS: Dict[ErrorCode, int] = {
    # 通用错误
    ErrorCode.INTERNAL_ERROR: 500,
    ErrorCode.INVALID_PARAMETER: 400,
    ErrorCode.MISSING_PARAMETER: 400,
    ErrorCode.INVALID_REQUEST: 400,
    ErrorCode.OPERATION_TIMEOUT: 408,
    ErrorCode.RATE_LIMIT_EXCEEDED: 429,
    
    # 服务相关
    ErrorCode.SERVICE_NOT_FOUND: 404,
    ErrorCode.SERVICE_ALREADY_EXISTS: 409,
    ErrorCode.SERVICE_UNAVAILABLE: 503,
    ErrorCode.SERVICE_TIMEOUT: 408,
    ErrorCode.SERVICE_CONNECTION_FAILED: 503,
    ErrorCode.SERVICE_INITIALIZATION_FAILED: 500,
    ErrorCode.SERVICE_CONFIGURATION_INVALID: 400,
    
    # 工具相关
    ErrorCode.TOOL_NOT_FOUND: 404,
    ErrorCode.TOOL_EXECUTION_FAILED: 500,
    ErrorCode.TOOL_PARAMETER_INVALID: 400,
    ErrorCode.TOOL_TIMEOUT: 408,
    ErrorCode.TOOL_UNAVAILABLE: 503,
    
    # Agent相关
    ErrorCode.AGENT_NOT_FOUND: 404,
    ErrorCode.AGENT_ALREADY_EXISTS: 409,
    ErrorCode.AGENT_OPERATION_FAILED: 500,
    
    # 配置相关
    ErrorCode.CONFIG_NOT_FOUND: 404,
    ErrorCode.CONFIG_INVALID: 400,
    ErrorCode.CONFIG_UPDATE_FAILED: 500,
    
    # 认证相关
    ErrorCode.AUTHENTICATION_REQUIRED: 401,
    ErrorCode.AUTHENTICATION_FAILED: 401,
    ErrorCode.AUTHORIZATION_FAILED: 403,
    ErrorCode.TOKEN_EXPIRED: 401,
    ErrorCode.TOKEN_INVALID: 401,
}

_ERROR_CODE_DESCRIPTIONS: Dict[ErrorCode, str] = {
    # 通用错误
    ErrorCode.INTERNAL_ERROR: "An unexpected internal server error occurred",
    ErrorCode.INVALID_PARAMETER: "One or more parameters are invalid",
    ErrorCode.MISSING_PARAMETER: "A required parameter is missing",
    ErrorCode.INVALID_REQUEST: "The request format is invalid",
    ErrorCode.OPERATION_TIMEOUT: "The operation timed out",
    ErrorCode.RATE_LIMIT_EXCEEDED: "Rate limit exceeded, please try again later",
    
    # 服务相关
    ErrorCode.SERVICE_NOT_FOUND: "The requested service does not exist",
    ErrorCode.SERVICE_ALREADY_EXISTS: "A service with this name already exists",
    ErrorCode.SERVICE_UNAVAILABLE: "The service is currently unavailable",
    ErrorCode.SERVICE_TIMEOUT: "Service connection or operation timed out",
    ErrorCode.SERVICE_CONNECTION_FAILED: "Failed to connect to the service",
    ErrorCode.SERVICE_INITIALIZATION_FAILED: "Service initialization failed",
    ErrorCode.SERVICE_CONFIGURATION_INVALID: "Service configuration is invalid",
    
    # 工具相关
    ErrorCode.TOOL_NOT_FOUND: "The requested tool does not exist",
    ErrorCode.TOOL_EXECUTION_FAILED: "Tool execution failed",
    ErrorCode.TOOL_PARAMETER_INVALID: "Tool parameters are invalid",
    ErrorCode.TOOL_TIMEOUT: "Tool execution timed out",
    ErrorCode.TOOL_UNAVAILABLE: "The tool is currently unavailable",
    
    # Agent相关
    ErrorCode.AGENT_NOT_FOUND: "The requested agent does not exist",
    ErrorCode.AGENT_ALREADY_EXISTS: "An agent with this ID already exists",
    ErrorCode.AGENT_OPERATION_FAILED: "Agent operation failed",
    
    # 配置相关
    ErrorCode.CONFIG_NOT_FOUND: "The requested configuration does not exist",
    ErrorCode.CONFIG_INVALID: "The configuration is invalid",
    ErrorCode.CONFIG_UPDATE_FAILED: "Failed to update configuration",
    
    # 认证相关
    ErrorCode.AUTHENTICATION_REQUIRED: "Authentication is required",
    ErrorCode.AUTHENTICATION_FAILED: "Authentication failed",
    ErrorCode.AUTHORIZATION_FAILED: "You do not have permission to perform this operation",
    ErrorCode.TOKEN_EXPIRED: "Your authentication token has expired",
    ErrorCode.TOKEN_INVALID: "The authentication token is invalid",
}

_ERROR_CODE_CATEGORIES: Dict[ErrorCode, str] = {
    # 通用错误
    ErrorCode.INTERNAL_ERROR: "General",
    ErrorCode.INVALID_PARAMETER: "General",
    ErrorCode.MISSING_PARAMETER: "General",
    ErrorCode.INVALID_REQUEST: "General",
    ErrorCode.OPERATION_TIMEOUT: "General",
    ErrorCode.RATE_LIMIT_EXCEEDED: "General",
    
    # 服务相关
    ErrorCode.SERVICE_NOT_FOUND: "Service",
    ErrorCode.SERVICE_ALREADY_EXISTS: "Service",
    ErrorCode.SERVICE_UNAVAILABLE: "Service",
    ErrorCode.SERVICE_TIMEOUT: "Service",
    ErrorCode.SERVICE_CONNECTION_FAILED: "Service",
    ErrorCode.SERVICE_INITIALIZATION_FAILED: "Service",
    ErrorCode.SERVICE_CONFIGURATION_INVALID: "Service",
    
    # 工具相关
    ErrorCode.TOOL_NOT_FOUND: "Tool",
    ErrorCode.TOOL_EXECUTION_FAILED: "Tool",
    ErrorCode.TOOL_PARAMETER_INVALID: "Tool",
    ErrorCode.TOOL_TIMEOUT: "Tool",
    ErrorCode.TOOL_UNAVAILABLE: "Tool",
    
    # Agent相关
    ErrorCode.AGENT_NOT_FOUND: "Agent",
    ErrorCode.AGENT_ALREADY_EXISTS: "Agent",
    ErrorCode.AGENT_OPERATION_FAILED: "Agent",
    
    # 配置相关
    ErrorCode.CONFIG_NOT_FOUND: "Configuration",
    ErrorCode.CONFIG_INVALID: "Configuration",
    ErrorCode.CONFIG_UPDATE_FAILED: "Configuration",
    
    # 认证相关
    ErrorCode.AUTHENTICATION_REQUIRED: "Authentication",
    ErrorCode.AUTHENTICATION_FAILED: "Authentication",
    ErrorCode.AUTHORIZATION_FAILED: "Authentication",
    ErrorCode.TOKEN_EXPIRED: "Authentication",
    ErrorCode.TOKEN_INVALID: "Authentication",
}

