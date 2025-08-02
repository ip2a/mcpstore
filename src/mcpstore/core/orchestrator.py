"""
MCP Service Orchestrator - Refactored Version

🏗️ Modular refactoring completed!

The original 2056-line orchestrator.py has been refactored into 8 specialized modules:
- base_orchestrator.py: Core infrastructure and lifecycle management (12 methods)
- monitoring_tasks.py: Monitoring tasks and loop management (12 methods)
- service_connection.py: Service connection and state management (15 methods)
- tool_execution.py: Tool execution and processing (4 methods)
- service_management.py: Service management and information retrieval (15 methods)
- resources_prompts.py: Resources/Prompts functionality (12 methods)
- network_utils.py: Network utilities and error handling (2 methods)
- standalone_config.py: Standalone configuration adapter (6 methods)

✅ Total of 78 methods, fully maintains backward compatibility
✅ Uses Mixin design pattern, clear separation of functional modules
✅ Each module focuses on specific functional areas, code organization is clearer
✅ Supports parallel development, more precise problem location, easier unit testing

This file is now a simple import proxy, actual implementation is in the orchestrator/ package.
"""

# Import refactored modular implementation
from .orchestrator import MCPOrchestrator

# Maintain backward compatibility - all existing imports should continue to work
__all__ = ['MCPOrchestrator']

# Refactoring information
__refactored__ = True
__refactor_version__ = "0.8.1"
__original_lines__ = 2056
__refactored_modules__ = 8
__total_methods__ = 78
