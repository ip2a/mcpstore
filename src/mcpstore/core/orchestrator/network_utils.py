"""
MCPOrchestrator Network Utils Module
Network utilities module - contains network error detection and utility methods
"""

import logging
from typing import Dict, Any

logger = logging.getLogger(__name__)

class NetworkUtilsMixin:
    """Network utilities mixin class"""

    def _is_network_error(self, error: Exception) -> bool:
        """Determine if it's a network-related error"""
        error_str = str(error).lower()
        network_error_keywords = [
            'connection', 'network', 'timeout', 'unreachable',
            'refused', 'reset', 'dns', 'resolve', 'socket'
        ]
        return any(keyword in error_str for keyword in network_error_keywords)

    def _is_filesystem_error(self, error: Exception) -> bool:
        """Determine if it's a filesystem-related error"""
        if isinstance(error, (FileNotFoundError, PermissionError, OSError, IOError)):
            return True

        error_str = str(error).lower()
        filesystem_error_keywords = [
            'no such file', 'file not found', 'permission denied',
            'access denied', 'directory not found', 'path not found'
        ]
        return any(keyword in error_str for keyword in filesystem_error_keywords)
