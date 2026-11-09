"""Context scope type enum."""

from enum import Enum


class ContextType(Enum):
    STORE = "store"
    AGENT = "agent"


__all__ = ["ContextType"]
