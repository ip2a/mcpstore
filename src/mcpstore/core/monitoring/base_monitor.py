"""
MCPStore monitoring and statistics module
Provides performance monitoring, tool usage statistics, alert management and other functions
"""

import json
import logging
import time
from dataclasses import dataclass
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List, Optional, Any

import aiohttp
import psutil

logger = logging.getLogger(__name__)

@dataclass
class PerformanceMetrics:
    """Performance metrics data class"""
    api_response_time: float  # API average response time (ms)
    active_connections: int   # Active connection count
    today_api_calls: int     # Today's API call count
    memory_usage: float      # Memory usage (%)
    cpu_usage: float         # CPU usage (%)
    uptime: float           # Uptime (seconds)

@dataclass
class ToolUsageStats:
    """工具使用统计数据类"""
    tool_name: str
    service_name: str
    execution_count: int
    last_executed: Optional[str]
    average_response_time: float
    success_rate: float

@dataclass
class ToolExecutionRecord:
    """工具执行记录数据类"""
    id: str
    tool_name: str
    service_name: str
    params: Dict[str, Any]
    result: Optional[Any]
    error: Optional[str]
    response_time: float  # 毫秒
    execution_time: str  # ISO格式时间戳
    timestamp: int  # Unix时间戳

@dataclass
class ToolRecordsSummary:
    """工具记录汇总数据类"""
    total_executions: int
    by_tool: Dict[str, Dict[str, Any]]  # tool_name -> {count, avg_response_time}
    by_service: Dict[str, Dict[str, Any]]  # service_name -> {count, avg_response_time}

@dataclass
class ToolRecordsResponse:
    """工具记录响应数据类"""
    executions: List[ToolExecutionRecord]
    summary: ToolRecordsSummary

@dataclass
class AlertInfo:
    """告警信息数据类"""
    alert_id: str
    type: str  # 'warning', 'error', 'info'
    title: str
    message: str
    timestamp: str
    service_name: Optional[str] = None
    resolved: bool = False

@dataclass
class NetworkEndpoint:
    """网络端点监控数据类"""
    endpoint_name: str
    url: str
    status: str  # 'healthy', 'warning', 'error'
    response_time: float
    last_checked: str
    uptime_percentage: float

@dataclass
class SystemResourceInfo:
    """系统资源信息数据类"""
    server_uptime: str
    memory_total: int
    memory_used: int
    memory_percentage: float
    disk_usage_percentage: float
    network_traffic_in: int
    network_traffic_out: int

class MonitoringManager:
    """监控管理器"""

    def __init__(self, data_dir: Path, tool_record_max_file_size: int = 30, tool_record_retention_days: int = 7):
        self.data_dir = data_dir
        self.tool_records_file = data_dir / "tool_records.json"  # 新的工具记录文件

        # 工具记录配置
        self.max_file_size_mb = tool_record_max_file_size
        self.retention_days = tool_record_retention_days

        # 记录启动时间用于计算运行时间
        self.start_time = time.time()

        # 确保数据文件存在
        self._ensure_data_files()
    
    def _ensure_data_files(self):
        """确保数据文件存在"""
        self.data_dir.mkdir(parents=True, exist_ok=True)

        if not self.tool_records_file.exists():
            initial_data = {
                "executions": [],
                "summary": {
                    "total_executions": 0,
                    "by_tool": {},
                    "by_service": {}
                }
            }
            self.tool_records_file.write_text(json.dumps(initial_data, indent=2))
    

    
    # 旧的record_tool_execution方法已移除，使用record_tool_execution_detailed代替
    

    

    
    # 旧的get_tool_usage_stats方法已移除，使用get_tool_records代替
    


    
    async def check_network_endpoints(self, endpoints: List[Dict[str, str]]) -> List[NetworkEndpoint]:
        """检查网络端点状态"""
        results = []
        
        async with aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=5)) as session:
            for endpoint in endpoints:
                name = endpoint.get("name", "Unknown")
                url = endpoint.get("url", "")
                
                start_time = time.time()
                status = "error"
                response_time = 0
                
                try:
                    async with session.get(url) as response:
                        response_time = (time.time() - start_time) * 1000
                        if response.status == 200:
                            status = "healthy" if response_time < 1000 else "warning"
                        else:
                            status = "warning"
                except:
                    status = "error"
                    response_time = 5000  # 超时
                
                results.append(NetworkEndpoint(
                    endpoint_name=name,
                    url=url,
                    status=status,
                    response_time=round(response_time, 2),
                    last_checked=datetime.now().isoformat(),
                    uptime_percentage=95.0  # 简化实现，实际应该基于历史数据
                ))
        
        return results
    
    def get_system_resource_info(self) -> SystemResourceInfo:
        """获取系统资源信息"""
        # 内存信息
        memory = psutil.virtual_memory()
        
        # 磁盘信息
        disk = psutil.disk_usage('/')
        
        # 网络信息
        net_io = psutil.net_io_counters()
        
        # 运行时间
        uptime_seconds = time.time() - self.start_time
        uptime_str = str(timedelta(seconds=int(uptime_seconds)))
        
        return SystemResourceInfo(
            server_uptime=uptime_str,
            memory_total=memory.total,
            memory_used=memory.used,
            memory_percentage=round(memory.percent, 1),
            disk_usage_percentage=round(disk.percent, 1),
            network_traffic_in=net_io.bytes_recv,
            network_traffic_out=net_io.bytes_sent
        )
    
    def increment_active_connections(self):
        """增加活跃连接数"""
        self.active_connections += 1
    
    def decrement_active_connections(self):
        """减少活跃连接数"""
        self.active_connections = max(0, self.active_connections - 1)

    def record_tool_execution_detailed(self, tool_name: str, service_name: str,
                                     params: Dict[str, Any], result: Optional[Any],
                                     error: Optional[str], response_time: float):
        """记录详细的工具执行信息"""
        try:
            # 读取现有数据
            with open(self.tool_records_file, 'r', encoding='utf-8') as f:
                data = json.load(f)

            # 创建新的执行记录
            execution_time = datetime.now()
            record = {
                "id": f"{int(execution_time.timestamp() * 1000)}_{hash(tool_name) % 10000:04d}",
                "tool_name": tool_name,
                "service_name": service_name,
                "params": params,
                "result": result,
                "error": error,
                "response_time": round(response_time, 2),
                "execution_time": execution_time.isoformat(),
                "timestamp": int(execution_time.timestamp())
            }

            # 添加到执行记录列表
            data["executions"].append(record)

            # 更新汇总统计
            self._update_summary(data, tool_name, service_name, response_time)

            # 清理过期数据
            self._cleanup_records(data)

            # 保存数据
            with open(self.tool_records_file, 'w', encoding='utf-8') as f:
                json.dump(data, f, indent=2, ensure_ascii=False)

        except Exception as e:
            logger.error(f"Failed to record detailed tool execution: {e}")

    def _update_summary(self, data: Dict, tool_name: str, service_name: str, response_time: float):
        """更新汇总统计"""
        summary = data["summary"]
        summary["total_executions"] += 1

        # 按工具统计
        if tool_name not in summary["by_tool"]:
            summary["by_tool"][tool_name] = {"count": 0, "total_response_time": 0.0}

        tool_stats = summary["by_tool"][tool_name]
        tool_stats["count"] += 1
        tool_stats["total_response_time"] += response_time
        tool_stats["avg_response_time"] = round(tool_stats["total_response_time"] / tool_stats["count"], 2)

        # 按服务统计
        if service_name not in summary["by_service"]:
            summary["by_service"][service_name] = {"count": 0, "total_response_time": 0.0}

        service_stats = summary["by_service"][service_name]
        service_stats["count"] += 1
        service_stats["total_response_time"] += response_time
        service_stats["avg_response_time"] = round(service_stats["total_response_time"] / service_stats["count"], 2)

    def _cleanup_records(self, data: Dict):
        """清理过期记录"""
        if self.max_file_size_mb == -1 and self.retention_days == -1:
            return  # 不清理

        executions = data["executions"]
        current_time = datetime.now()

        # 按时间清理（如果设置了保留天数）
        if self.retention_days != -1:
            cutoff_timestamp = int((current_time - timedelta(days=self.retention_days)).timestamp())
            executions = [e for e in executions if e["timestamp"] >= cutoff_timestamp]

        # 按文件大小清理（如果设置了最大文件大小）
        if self.max_file_size_mb != -1:
            # 检查当前文件大小
            current_size_mb = self.tool_records_file.stat().st_size / (1024 * 1024)
            if current_size_mb > self.max_file_size_mb:
                # 保留最新的记录，删除最旧的
                target_count = int(len(executions) * 0.8)  # 保留80%的记录
                executions = sorted(executions, key=lambda x: x["timestamp"], reverse=True)[:target_count]

        # 更新数据
        data["executions"] = executions

        # 重新计算汇总统计
        self._recalculate_summary(data)

    def _recalculate_summary(self, data: Dict):
        """重新计算汇总统计"""
        executions = data["executions"]
        summary = {
            "total_executions": len(executions),
            "by_tool": {},
            "by_service": {}
        }

        # 重新统计
        for execution in executions:
            tool_name = execution["tool_name"]
            service_name = execution["service_name"]
            response_time = execution["response_time"]

            # 按工具统计
            if tool_name not in summary["by_tool"]:
                summary["by_tool"][tool_name] = {"count": 0, "total_response_time": 0.0}

            tool_stats = summary["by_tool"][tool_name]
            tool_stats["count"] += 1
            tool_stats["total_response_time"] += response_time
            tool_stats["avg_response_time"] = round(tool_stats["total_response_time"] / tool_stats["count"], 2)

            # 按服务统计
            if service_name not in summary["by_service"]:
                summary["by_service"][service_name] = {"count": 0, "total_response_time": 0.0}

            service_stats = summary["by_service"][service_name]
            service_stats["count"] += 1
            service_stats["total_response_time"] += response_time
            service_stats["avg_response_time"] = round(service_stats["total_response_time"] / service_stats["count"], 2)

        data["summary"] = summary

    def get_tool_records(self, limit: int = 50) -> Dict[str, Any]:
        """获取工具执行记录"""
        try:
            with open(self.tool_records_file, 'r', encoding='utf-8') as f:
                data = json.load(f)

            # 按时间戳倒序排列，返回最新的记录
            executions = sorted(data["executions"], key=lambda x: x["timestamp"], reverse=True)
            if limit > 0:
                executions = executions[:limit]

            return {
                "executions": executions,
                "summary": data["summary"]
            }

        except Exception as e:
            logger.error(f"Failed to get tool records: {e}")
            return {
                "executions": [],
                "summary": {
                    "total_executions": 0,
                    "by_tool": {},
                    "by_service": {}
                }
            }
