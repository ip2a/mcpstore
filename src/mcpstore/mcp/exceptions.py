"""Custom exceptions for MCPStore."""

from mcp import McpError  # noqa: F401


class MCPStoreError(Exception):
    """Base error for MCPStore."""


class ValidationError(MCPStoreError):
    """Error in validating parameters or return values."""


class ResourceError(MCPStoreError):
    """Error in resource operations."""


class ToolError(MCPStoreError):
    """Error in tool operations."""


class PromptError(MCPStoreError):
    """Error in prompt operations."""


class InvalidSignature(Exception):
    """Invalid signature for use with MCPStore."""


class ClientError(Exception):
    """Error in client operations."""


class NotFoundError(Exception):
    """Object not found."""


class DisabledError(Exception):
    """Object is disabled."""


class AuthorizationError(MCPStoreError):
    """Error when authorization check fails."""
