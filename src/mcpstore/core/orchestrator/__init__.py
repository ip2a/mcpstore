"""
MCPOrchestrator Package
Orchestrator package - Modularized refactored MCP service orchestrator

This package refactors the original 2056-line orchestrator.py into 8 specialized modules:
- base_orchestrator.py: Core infrastructure and lifecycle management (12 methods)
- monitoring_tasks.py: Monitoring tasks and loop management (12 methods)
- service_connection.py: Service connection and state management (15 methods)
- tool_execution.py: Tool execution and processing (4 methods)
- service_management.py: Service management and information retrieval (15 methods)
- resources_prompts.py: Resources/Prompts functionality (12 methods)
- network_utils.py: Network utilities and error handling (2 methods)
- standalone_config.py: Standalone configuration adapter (6 methods)

Total of 78 methods, fully maintaining backward compatibility.
"""

from .base_orchestrator import MCPOrchestrator

# Export main classes
__all__ = ['MCPOrchestrator']

# Version information
__version__ = "0.8.1"
__description__ = "Modular MCP Service Orchestrator"
