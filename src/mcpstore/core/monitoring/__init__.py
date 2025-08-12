"""
MCPStore Monitoring Module
Monitoring module

Responsible for tool monitoring, performance analysis, metrics collection and monitoring configuration
"""

# Main exports - maintain backward compatibility
from .tools_monitor import ToolsUpdateMonitor
from .message_handler import MCPStoreMessageHandler
try:
    from .analytics import MonitoringAnalytics, EventCollector, ToolUsageMetrics, ServiceHealthMetrics
except ImportError:
    # If analytics module import fails, provide placeholder
    MonitoringAnalytics = None
    EventCollector = None
    ToolUsageMetrics = None
    ServiceHealthMetrics = None

try:
    from .base_monitor import MonitoringManager, NetworkEndpoint, SystemResourceInfo
    BaseMonitor = MonitoringManager  # For backward compatibility
except ImportError as e:
    print(f"Warning: Failed to import from base_monitor: {e}")
    BaseMonitor = None
    MonitoringManager = None
    NetworkEndpoint = None
    SystemResourceInfo = None

try:
    from .config import MonitoringConfig
except ImportError:
    MonitoringConfig = None

__all__ = [
    'ToolsUpdateMonitor',
    'MCPStoreMessageHandler',
    'MonitoringAnalytics',
    'EventCollector',
    'ToolUsageMetrics',
    'ServiceHealthMetrics',
    'BaseMonitor',
    'MonitoringManager',
    'NetworkEndpoint',
    'SystemResourceInfo',
    'MonitoringConfig'
]
