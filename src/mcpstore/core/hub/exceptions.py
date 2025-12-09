"""
Hub MCP Exceptions Module
Hub MCP 异常模块 - 定义 Hub MCP 相关的异常类
"""


class HubMCPError(Exception):
    """
    Hub MCP 错误基类
    
    所有 Hub MCP 相关的异常都继承自此类。
    """
    pass


class ServerAlreadyRunningError(HubMCPError):
    """
    服务器已在运行错误
    
    当尝试启动一个已经在运行的服务器时抛出。
    """
    pass


class ServerNotRunningError(HubMCPError):
    """
    服务器未运行错误
    
    当尝试对未运行的服务器执行操作时抛出。
    """
    pass


class ToolExecutionError(HubMCPError):
    """
    工具执行错误
    
    当工具调用失败时抛出。
    """
    pass


class PortBindingError(HubMCPError):
    """
    端口绑定错误
    
    当无法绑定到指定端口时抛出。
    """
    pass
