"""
MCP Store exception class definitions
"""

class MCPStoreError(Exception):
    """MCP Store base exception class"""
    pass

class ServiceNotFoundError(MCPStoreError):
    """Service does not exist"""
    pass

class InvalidConfigError(MCPStoreError):
    """Invalid configuration"""
    pass

class DeleteServiceError(MCPStoreError):
    """Failed to delete service"""
    pass

