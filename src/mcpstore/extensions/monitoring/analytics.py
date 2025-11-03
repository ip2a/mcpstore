#!/usr/bin/env python3
"""
Monitoring and Analytics Features
Tool usage analysis, performance dashboard, error tracking, usage report generation
"""

import json
import logging
import statistics
from collections import defaultdict, deque
from dataclasses import dataclass, field, asdict
from datetime import datetime, timedelta
from enum import Enum
from pathlib import Path
from typing import Dict, List, Any, Optional

logger = logging.getLogger(__name__)

class EventType(Enum):
    """Event types"""
    TOOL_EXECUTION = "tool_execution"
    SERVICE_CONNECTION = "service_connection"
    ERROR = "error"
    PERFORMANCE = "performance"
    USER_ACTION = "user_action"
    SYSTEM = "system"

class Severity(Enum):
    """Severity levels"""
    DEBUG = "debug"
    INFO = "info"
    WARNING = "warning"
    ERROR = "error"
    CRITICAL = "critical"

@dataclass
class Event:
    """Event record"""
    event_id: str
    event_type: EventType
    timestamp: datetime
    severity: Severity
    message: str
    data: Dict[str, Any] = field(default_factory=dict)
    user_id: Optional[str] = None
    service_name: Optional[str] = None
    tool_name: Optional[str] = None
    duration: Optional[float] = None
    success: bool = True

@dataclass
class ToolUsageMetrics:
    """工具使用指标"""
    tool_name: str
    service_name: str
    total_calls: int = 0
    successful_calls: int = 0
    failed_calls: int = 0
    total_duration: float = 0.0
    avg_duration: float = 0.0
    min_duration: float = float('inf')
    max_duration: float = 0.0
    last_used: Optional[datetime] = None
    error_rate: float = 0.0
    
    def update(self, duration: float, success: bool):
        """更新指标"""
        self.total_calls += 1
        self.total_duration += duration
        self.last_used = datetime.now()
        
        if success:
            self.successful_calls += 1
        else:
            self.failed_calls += 1
        
        self.avg_duration = self.total_duration / self.total_calls
        self.min_duration = min(self.min_duration, duration)
        self.max_duration = max(self.max_duration, duration)
        self.error_rate = self.failed_calls / self.total_calls

@dataclass
class ServiceHealthMetrics:
    """服务健康指标"""
    service_name: str
    status: str = "unknown"
    uptime: float = 0.0
    response_time: float = 0.0
    error_count: int = 0
    last_check: Optional[datetime] = None
    connection_count: int = 0
    
class EventCollector:
    """事件收集器"""
    
    def __init__(self, max_events: int = 10000):
        self.max_events = max_events
        self._events: deque = deque(maxlen=max_events)
        self._event_counter = 0
    
    def record_event(self, event: Event):
        """Record event"""
        event.event_id = f"evt_{self._event_counter:06d}"
        self._event_counter += 1
        self._events.append(event)
        
        # Record to log
        log_level = {
            Severity.DEBUG: logging.DEBUG,
            Severity.INFO: logging.INFO,
            Severity.WARNING: logging.WARNING,
            Severity.ERROR: logging.ERROR,
            Severity.CRITICAL: logging.CRITICAL
        }.get(event.severity, logging.INFO)
        
        logger.log(log_level, f"[{event.event_type.value}] {event.message}")
    
    def get_events(
        self,
        event_type: Optional[EventType] = None,
        severity: Optional[Severity] = None,
        since: Optional[datetime] = None,
        limit: Optional[int] = None
    ) -> List[Event]:
        """Get events"""
        events = list(self._events)

        # Filter conditions
        if event_type:
            events = [e for e in events if e.event_type == event_type]
        
        if severity:
            events = [e for e in events if e.severity == severity]
        
        if since:
            events = [e for e in events if e.timestamp >= since]
        
        # Sort by time in descending order
        events.sort(key=lambda e: e.timestamp, reverse=True)
        
        if limit:
            events = events[:limit]
        
        return events
    
    def get_error_events(self, hours: int = 24) -> List[Event]:
        """Get error events"""
        since = datetime.now() - timedelta(hours=hours)
        return self.get_events(
            severity=Severity.ERROR,
            since=since
        )

class MetricsCollector:
    """Metrics collector"""
    
    def __init__(self):
        self._tool_metrics: Dict[str, ToolUsageMetrics] = {}
        self._service_metrics: Dict[str, ServiceHealthMetrics] = {}
        self._performance_data: Dict[str, deque] = defaultdict(lambda: deque(maxlen=1000))
    
    def record_tool_execution(
        self,
        tool_name: str,
        service_name: str,
        duration: float,
        success: bool,
        user_id: Optional[str] = None
    ):
        """Record tool execution"""
        key = f"{service_name}:{tool_name}"
        
        if key not in self._tool_metrics:
            self._tool_metrics[key] = ToolUsageMetrics(
                tool_name=tool_name,
                service_name=service_name
            )
        
        self._tool_metrics[key].update(duration, success)
        
        # Record performance data
        self._performance_data[key].append({
            "timestamp": datetime.now(),
            "duration": duration,
            "success": success,
            "user_id": user_id
        })
    
    def update_service_health(
        self,
        service_name: str,
        status: str,
        response_time: float = 0.0,
        error_count: int = 0
    ):
        """Update service health status"""
        if service_name not in self._service_metrics:
            self._service_metrics[service_name] = ServiceHealthMetrics(
                service_name=service_name
            )
        
        metrics = self._service_metrics[service_name]
        metrics.status = status
        metrics.response_time = response_time
        metrics.error_count = error_count
        metrics.last_check = datetime.now()
    
    def get_tool_metrics(self, tool_name: Optional[str] = None) -> Dict[str, ToolUsageMetrics]:
        """获取工具指标"""
        if tool_name:
            return {k: v for k, v in self._tool_metrics.items() if tool_name in k}
        return self._tool_metrics.copy()
    
    def get_service_health(self, service_name: Optional[str] = None) -> Dict[str, ServiceHealthMetrics]:
        """获取服务健康状态"""
        if service_name:
            return {k: v for k, v in self._service_metrics.items() if k == service_name}
        return self._service_metrics.copy()
    
    def get_top_tools(self, limit: int = 10) -> List[ToolUsageMetrics]:
        """获取最常用的工具"""
        tools = list(self._tool_metrics.values())
        tools.sort(key=lambda t: t.total_calls, reverse=True)
        return tools[:limit]
    
    def get_performance_trends(self, tool_name: str, hours: int = 24) -> Dict[str, Any]:
        """获取性能趋势"""
        key = None
        for k in self._performance_data.keys():
            if tool_name in k:
                key = k
                break
        
        if not key:
            return {}
        
        data = list(self._performance_data[key])
        since = datetime.now() - timedelta(hours=hours)
        recent_data = [d for d in data if d["timestamp"] >= since]
        
        if not recent_data:
            return {}
        
        durations = [d["duration"] for d in recent_data]
        success_rate = sum(1 for d in recent_data if d["success"]) / len(recent_data)
        
        return {
            "tool_name": tool_name,
            "period_hours": hours,
            "total_calls": len(recent_data),
            "success_rate": success_rate,
            "avg_duration": statistics.mean(durations),
            "median_duration": statistics.median(durations),
            "min_duration": min(durations),
            "max_duration": max(durations),
            "std_duration": statistics.stdev(durations) if len(durations) > 1 else 0
        }

class ErrorTracker:
    """错误追踪器"""
    
    def __init__(self):
        self._error_patterns: Dict[str, int] = defaultdict(int)
        self._error_details: List[Dict[str, Any]] = []
    
    def track_error(
        self,
        error: Exception,
        context: Dict[str, Any] = None,
        tool_name: Optional[str] = None,
        service_name: Optional[str] = None
    ):
        """追踪错误"""
        error_type = type(error).__name__
        error_message = str(error)
        
        # 记录错误模式
        pattern_key = f"{error_type}:{tool_name or 'unknown'}"
        self._error_patterns[pattern_key] += 1
        
        # 记录错误详情
        error_detail = {
            "timestamp": datetime.now(),
            "error_type": error_type,
            "error_message": error_message,
            "tool_name": tool_name,
            "service_name": service_name,
            "context": context or {},
            "count": self._error_patterns[pattern_key]
        }
        
        self._error_details.append(error_detail)
        
        # 保持最近的1000个错误
        if len(self._error_details) > 1000:
            self._error_details.pop(0)
    
    def get_error_summary(self, hours: int = 24) -> Dict[str, Any]:
        """获取错误摘要"""
        since = datetime.now() - timedelta(hours=hours)
        recent_errors = [
            e for e in self._error_details
            if e["timestamp"] >= since
        ]
        
        if not recent_errors:
            return {"total_errors": 0, "error_types": {}, "top_errors": []}
        
        # 统计错误类型
        error_types = defaultdict(int)
        for error in recent_errors:
            error_types[error["error_type"]] += 1
        
        # 获取最常见的错误
        top_errors = sorted(
            self._error_patterns.items(),
            key=lambda x: x[1],
            reverse=True
        )[:10]
        
        return {
            "total_errors": len(recent_errors),
            "error_types": dict(error_types),
            "top_errors": [{"pattern": pattern, "count": count} for pattern, count in top_errors],
            "recent_errors": recent_errors[-10:]  # 最近10个错误
        }

class ReportGenerator:
    """报告生成器"""
    
    def __init__(self, metrics_collector: MetricsCollector, error_tracker: ErrorTracker):
        self.metrics_collector = metrics_collector
        self.error_tracker = error_tracker
    
    def generate_usage_report(self, hours: int = 24) -> Dict[str, Any]:
        """生成使用报告"""
        tool_metrics = self.metrics_collector.get_tool_metrics()
        service_health = self.metrics_collector.get_service_health()
        top_tools = self.metrics_collector.get_top_tools()
        error_summary = self.error_tracker.get_error_summary(hours)
        
        return {
            "report_period": f"{hours} hours",
            "generated_at": datetime.now().isoformat(),
            "summary": {
                "total_tools": len(tool_metrics),
                "total_services": len(service_health),
                "total_tool_calls": sum(m.total_calls for m in tool_metrics.values()),
                "total_errors": error_summary["total_errors"]
            },
            "top_tools": [asdict(tool) for tool in top_tools],
            "service_health": {name: asdict(health) for name, health in service_health.items()},
            "error_summary": error_summary
        }
    
    def save_report(self, report: Dict[str, Any], file_path: Optional[Path] = None):
        """保存报告到文件"""
        if not file_path:
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            file_path = Path(f"mcpstore_report_{timestamp}.json")
        
        try:
            with open(file_path, 'w', encoding='utf-8') as f:
                json.dump(report, f, indent=2, ensure_ascii=False, default=str)
            logger.info(f"Report saved to {file_path}")
        except Exception as e:
            logger.error(f"Failed to save report: {e}")

class MonitoringManager:
    """监控管理器"""
    
    def __init__(self):
        self.event_collector = EventCollector()
        self.metrics_collector = MetricsCollector()
        self.error_tracker = ErrorTracker()
        self.report_generator = ReportGenerator(self.metrics_collector, self.error_tracker)
    
    def record_tool_execution(
        self,
        tool_name: str,
        service_name: str,
        duration: float,
        success: bool,
        user_id: Optional[str] = None,
        error: Optional[Exception] = None
    ):
        """记录工具执行"""
        # 记录指标
        self.metrics_collector.record_tool_execution(
            tool_name, service_name, duration, success, user_id
        )
        
        # 记录事件
        event = Event(
            event_id="",  # 将由 event_collector 分配
            event_type=EventType.TOOL_EXECUTION,
            timestamp=datetime.now(),
            severity=Severity.INFO if success else Severity.ERROR,
            message=f"Tool {tool_name} {'succeeded' if success else 'failed'}",
            data={
                "duration": duration,
                "success": success
            },
            user_id=user_id,
            service_name=service_name,
            tool_name=tool_name,
            duration=duration,
            success=success
        )
        self.event_collector.record_event(event)
        
        # 记录错误
        if error:
            self.error_tracker.track_error(
                error,
                context={"tool_name": tool_name, "service_name": service_name},
                tool_name=tool_name,
                service_name=service_name
            )
    
    def get_dashboard_data(self) -> Dict[str, Any]:
        """获取仪表板数据"""
        return {
            "overview": {
                "total_tools": len(self.metrics_collector.get_tool_metrics()),
                "total_services": len(self.metrics_collector.get_service_health()),
                "recent_errors": len(self.event_collector.get_error_events(hours=1))
            },
            "top_tools": [asdict(tool) for tool in self.metrics_collector.get_top_tools(5)],
            "service_health": {
                name: asdict(health) 
                for name, health in self.metrics_collector.get_service_health().items()
            },
            "recent_events": [
                asdict(event) for event in self.event_collector.get_events(limit=10)
            ],
            "error_summary": self.error_tracker.get_error_summary(hours=24)
        }

# 全局实例
_global_monitoring_manager = None

def get_monitoring_manager() -> MonitoringManager:
    """获取全局监控管理器"""
    global _global_monitoring_manager
    if _global_monitoring_manager is None:
        _global_monitoring_manager = MonitoringManager()
    return _global_monitoring_manager
