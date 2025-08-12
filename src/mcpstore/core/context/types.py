"""
MCPStore Context Types
Context-related type definitions
"""

from enum import Enum

class ContextType(Enum):
    """Context type"""
    STORE = "store"
    AGENT = "agent"
