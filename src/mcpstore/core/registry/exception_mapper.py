"""
Exception Mapper for py-key-value Integration

This module provides utilities to map py-key-value exceptions to MCPStore exceptions,
ensuring consistent error handling across the codebase.

Validates: Requirements 6.4 (Exception and error handling)
"""

import logging
from functools import wraps
from typing import Any, Callable, TypeVar, ParamSpec

from ..exceptions import (
    CacheOperationError,
    CacheConnectionError,
    CacheValidationError,
)

logger = logging.getLogger(__name__)

# Type variables for generic decorator
P = ParamSpec('P')
T = TypeVar('T')


def map_kv_exception(func: Callable[P, T]) -> Callable[P, T]:
    """
    Decorator to map py-key-value exceptions to MCPStore exceptions.
    
    This decorator wraps async functions that interact with py-key-value storage
    and translates any py-key-value exceptions into appropriate MCPStore exceptions.
    
    Exception Mapping:
        - KeyValueOperationError → CacheOperationError
        - SerializationError → CacheValidationError (validation_type="serialization")
        - DeserializationError → CacheValidationError (validation_type="deserialization")
        - MissingKeyError → CacheValidationError (validation_type="missing_key")
        - InvalidTTLError → CacheValidationError (validation_type="invalid_ttl")
        - StoreConnectionError → CacheConnectionError
        - StoreSetupError → CacheConnectionError
        - BaseKeyValueError → CacheOperationError (fallback)
    
    Args:
        func: The async function to wrap
        
    Returns:
        Wrapped function that maps exceptions
        
    Example:
        @map_kv_exception
        async def get_tool_cache(self, agent_id: str) -> Dict[str, Any]:
            collection = self._get_collection(agent_id, "tools")
            return await self._kv_store.get(collection=collection)
    
    Validates: Requirements 6.4 (Exception and error handling)
    """
    @wraps(func)
    async def wrapper(*args: P.args, **kwargs: P.kwargs) -> T:
        try:
            return await func(*args, **kwargs)
        except Exception as e:
            # Try to import py-key-value exceptions
            try:
                from key_value.shared.errors.base import BaseKeyValueError
                from key_value.shared.errors.key_value import (
                    KeyValueOperationError,
                    SerializationError,
                    DeserializationError,
                    MissingKeyError,
                    InvalidTTLError,
                )
                from key_value.shared.errors.store import (
                    StoreConnectionError,
                    StoreSetupError,
                )
            except ImportError:
                # If py-key-value is not installed, just re-raise
                logger.warning("py-key-value not installed, cannot map exceptions")
                raise
            
            # Extract context information for logging
            context_info = f"operation={func.__name__}"
            if args:
                # Try to extract agent_id if it's the first argument after self
                if len(args) > 1 and isinstance(args[1], str):
                    context_info += f", agent_id={args[1]}"
            
            # Map py-key-value exceptions to MCPStore exceptions
            if isinstance(e, SerializationError):
                logger.error(f"Cache serialization error: {e} ({context_info})")
                raise CacheValidationError(
                    message=f"Failed to serialize data: {e}",
                    validation_type="serialization",
                    cause=e
                ) from e
            
            elif isinstance(e, DeserializationError):
                logger.error(f"Cache deserialization error: {e} ({context_info})")
                raise CacheValidationError(
                    message=f"Failed to deserialize data: {e}",
                    validation_type="deserialization",
                    cause=e
                ) from e
            
            elif isinstance(e, MissingKeyError):
                logger.error(f"Cache missing key error: {e} ({context_info})")
                raise CacheValidationError(
                    message=f"Missing cache key: {e}",
                    validation_type="missing_key",
                    cause=e
                ) from e
            
            elif isinstance(e, InvalidTTLError):
                logger.error(f"Cache invalid TTL error: {e} ({context_info})")
                raise CacheValidationError(
                    message=f"Invalid TTL for cache: {e}",
                    validation_type="invalid_ttl",
                    cause=e
                ) from e
            
            elif isinstance(e, (StoreConnectionError, StoreSetupError)):
                logger.error(f"Cache connection error: {e} ({context_info})")
                raise CacheConnectionError(
                    message=f"Cache connection failed: {e}",
                    backend_type=type(e).__name__,
                    cause=e
                ) from e
            
            elif isinstance(e, KeyValueOperationError):
                logger.error(f"Cache operation error: {e} ({context_info})")
                raise CacheOperationError(
                    message=f"Cache operation failed: {e}",
                    operation=func.__name__,
                    cause=e
                ) from e
            
            elif isinstance(e, BaseKeyValueError):
                # Fallback for any other py-key-value exceptions
                logger.error(f"Cache error: {e} ({context_info})")
                raise CacheOperationError(
                    message=f"Cache error: {e}",
                    operation=func.__name__,
                    cause=e
                ) from e
            
            else:
                # Not a py-key-value exception, re-raise as-is
                raise
    
    return wrapper


def validate_session_serializable(session: Any, agent_id: str, service_name: str) -> None:
    """
    Validate that a Session object does not contain non-serializable references.
    
    This function performs defensive checks to ensure Session objects remain
    in memory and are never accidentally serialized to py-key-value storage.
    
    Args:
        session: The Session object to validate
        agent_id: Agent ID for error reporting
        service_name: Service name for error reporting
        
    Raises:
        SessionSerializationError: If the session contains non-serializable references
        
    Validates: Requirements 3.2 (Session object serialization issues)
    """
    from ..exceptions import SessionSerializationError
    
    if session is None:
        return
    
    # Check for common non-serializable attributes
    non_serializable_attrs = [
        '_kv_store',      # py-key-value store reference
        '_connection',    # Connection objects
        '_socket',        # Socket objects
        '_stream',        # Stream objects
        '_transport',     # Transport objects
        '_protocol',      # Protocol objects
        'session',        # Nested session objects
        'client',         # Client objects
    ]
    
    if hasattr(session, '__dict__'):
        session_attrs = set(session.__dict__.keys())
        found_attrs = [attr for attr in non_serializable_attrs if attr in session_attrs]
        
        if found_attrs:
            raise SessionSerializationError(
                message=(
                    f"Session object contains non-serializable references: {found_attrs}. "
                    f"Session objects should remain in memory and never be serialized. "
                    f"Agent: {agent_id}, Service: {service_name}"
                ),
                session_info={
                    "agent_id": agent_id,
                    "service_name": service_name,
                    "non_serializable_attrs": found_attrs,
                    "session_type": type(session).__name__,
                }
            )
    
    # Check if session has a to_dict or dict method (might indicate serialization attempt)
    if hasattr(session, 'to_dict') or hasattr(session, 'dict'):
        logger.warning(
            f"Session object has serialization methods (to_dict/dict). "
            f"Ensure it's not being serialized. Agent: {agent_id}, Service: {service_name}"
        )
    
    logger.debug(
        f"Session validation passed for agent={agent_id}, service={service_name}, "
        f"type={type(session).__name__}"
    )


def safe_session_operation(func: Callable[P, T]) -> Callable[P, T]:
    """
    Decorator to ensure Session operations are safe and don't attempt serialization.
    
    This decorator wraps Session-related operations and validates that Session objects
    are not being accidentally serialized or stored in py-key-value.
    
    Args:
        func: The function to wrap (can be sync or async)
        
    Returns:
        Wrapped function with Session validation
        
    Example:
        @safe_session_operation
        def set_session(self, agent_id: str, service_name: str, session: Any) -> None:
            if agent_id not in self.sessions:
                self.sessions[agent_id] = {}
            self.sessions[agent_id][service_name] = session
    
    Validates: Requirements 3.2 (Session object serialization issues)
    """
    @wraps(func)
    def wrapper(*args: P.args, **kwargs: P.kwargs) -> T:
        # Extract session, agent_id, and service_name from arguments
        # Assume common patterns: (self, agent_id, service_name, session) or similar
        session = None
        agent_id = None
        service_name = None
        
        # Try to extract from positional args
        if len(args) >= 4:
            # Pattern: (self, agent_id, service_name, session)
            agent_id = args[1] if isinstance(args[1], str) else None
            service_name = args[2] if isinstance(args[2], str) else None
            session = args[3]
        elif len(args) >= 3:
            # Pattern: (self, agent_id, service_name) - getting session
            agent_id = args[1] if isinstance(args[1], str) else None
            service_name = args[2] if isinstance(args[2], str) else None
        
        # Try to extract from kwargs
        if 'session' in kwargs:
            session = kwargs['session']
        if 'agent_id' in kwargs:
            agent_id = kwargs['agent_id']
        if 'service_name' in kwargs:
            service_name = kwargs['service_name']
        
        # Validate session if we're setting it
        if session is not None and agent_id and service_name:
            validate_session_serializable(session, agent_id, service_name)
        
        # Call the original function
        return func(*args, **kwargs)
    
    return wrapper
