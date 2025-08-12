"""
MCPStore API Response Models
Contains request and response models used by all API endpoints
"""

from typing import Optional, List, Dict, Any

from pydantic import BaseModel, Field


# === Monitoring-related response models ===

class ToolUsageStatsResponse(BaseModel):
    """Tool usage statistics response"""
    tool_name: str = Field(description="Tool name")
    service_name: str = Field(description="Service name")
    execution_count: int = Field(description="Execution count")
    last_executed: Optional[str] = Field(description="Last execution time")
    average_response_time: float = Field(description="Average response time")
    success_rate: float = Field(description="Success rate")

class ToolExecutionRecordResponse(BaseModel):
    """Tool execution record response"""
    id: str = Field(description="Record ID")
    tool_name: str = Field(description="Tool name")
    service_name: str = Field(description="Service name")
    params: Dict[str, Any] = Field(description="Execution parameters")
    result: Optional[Any] = Field(description="Execution result")
    error: Optional[str] = Field(description="Error message")
    response_time: float = Field(description="Response time (milliseconds)")
    execution_time: str = Field(description="Execution time")
    timestamp: int = Field(description="Timestamp")

class ToolRecordsSummaryResponse(BaseModel):
    """工具记录汇总响应"""
    total_executions: int = Field(description="总执行次数")
    by_tool: Dict[str, Dict[str, Any]] = Field(description="按工具统计")
    by_service: Dict[str, Dict[str, Any]] = Field(description="按服务统计")

class ToolRecordsResponse(BaseModel):
    """工具记录完整响应"""
    executions: List[ToolExecutionRecordResponse] = Field(description="执行记录列表")
    summary: ToolRecordsSummaryResponse = Field(description="汇总统计")

class NetworkEndpointResponse(BaseModel):
    """网络端点响应"""
    endpoint_name: str = Field(description="端点名称")
    url: str = Field(description="端点URL")
    status: str = Field(description="状态")
    response_time: float = Field(description="响应时间")
    last_checked: str = Field(description="最后检查时间")
    uptime_percentage: float = Field(description="可用性百分比")

class SystemResourceInfoResponse(BaseModel):
    """系统资源信息响应"""
    server_uptime: str = Field(description="服务器运行时间")
    memory_total: int = Field(description="总内存")
    memory_used: int = Field(description="已用内存")
    memory_percentage: float = Field(description="内存使用率")
    disk_usage_percentage: float = Field(description="磁盘使用率")
    network_traffic_in: int = Field(description="网络入流量")
    network_traffic_out: int = Field(description="网络出流量")

class AddAlertRequest(BaseModel):
    """添加告警请求"""
    type: str = Field(description="告警类型: warning, error, info")
    title: str = Field(description="告警标题")
    message: str = Field(description="告警消息")
    service_name: Optional[str] = Field(None, description="相关服务名称")

class NetworkEndpointCheckRequest(BaseModel):
    """网络端点检查请求"""
    endpoints: List[Dict[str, str]] = Field(description="端点列表")

# === 健康状态相关响应模型 ===
class ServiceHealthResponse(BaseModel):
    """服务健康状态响应"""
    service_name: str = Field(description="服务名称")
    status: str = Field(description="服务状态: initializing, healthy, warning, reconnecting, unreachable, disconnecting, disconnected")
    response_time: float = Field(description="最近响应时间（秒）")
    last_check_time: float = Field(description="最后检查时间戳")
    consecutive_failures: int = Field(description="连续失败次数")
    consecutive_successes: int = Field(description="连续成功次数")
    reconnect_attempts: int = Field(description="重连尝试次数")
    state_entered_time: Optional[str] = Field(None, description="状态进入时间")
    next_retry_time: Optional[str] = Field(None, description="下次重试时间")
    error_message: Optional[str] = Field(None, description="错误信息")
    details: Dict[str, Any] = Field(default_factory=dict, description="详细信息")

class HealthSummaryResponse(BaseModel):
    """健康状态汇总响应"""
    total_services: int = Field(description="总服务数量")
    initializing_count: int = Field(description="初始化中服务数量")
    healthy_count: int = Field(description="健康服务数量")
    warning_count: int = Field(description="警告状态服务数量")
    reconnecting_count: int = Field(description="重连中服务数量")
    unreachable_count: int = Field(description="无法访问服务数量")
    disconnecting_count: int = Field(description="断连中服务数量")
    disconnected_count: int = Field(description="已断连服务数量")
    services: Dict[str, ServiceHealthResponse] = Field(description="各服务健康状态详情")

# === Agent统计相关响应模型 ===
class AgentServiceSummaryResponse(BaseModel):
    """Agent服务摘要响应"""
    service_name: str = Field(description="服务名称")
    service_type: str = Field(description="服务类型")
    status: str = Field(description="服务状态: initializing, healthy, warning, reconnecting, unreachable, disconnecting, disconnected")
    tool_count: int = Field(description="工具数量")
    last_used: Optional[str] = Field(None, description="最后使用时间")
    client_id: Optional[str] = Field(None, description="客户端ID")
    response_time: Optional[float] = Field(None, description="最近响应时间（秒）")
    health_details: Optional[Dict[str, Any]] = Field(None, description="健康状态详情")

class AgentStatisticsResponse(BaseModel):
    """Agent统计信息响应"""
    agent_id: str = Field(description="Agent ID")
    service_count: int = Field(description="服务数量")
    tool_count: int = Field(description="工具数量")
    healthy_services: int = Field(description="健康服务数量")
    unhealthy_services: int = Field(description="不健康服务数量")
    total_tool_executions: int = Field(description="总工具执行次数")
    last_activity: Optional[str] = Field(None, description="最后活动时间")
    services: List[AgentServiceSummaryResponse] = Field(description="服务列表")

class AgentsSummaryResponse(BaseModel):
    """所有Agent汇总信息响应"""
    total_agents: int = Field(description="总Agent数量")
    active_agents: int = Field(description="活跃Agent数量")
    total_services: int = Field(description="总服务数量")
    total_tools: int = Field(description="总工具数量")
    store_services: int = Field(description="Store级别服务数量")
    store_tools: int = Field(description="Store级别工具数量")
    agents: List[AgentStatisticsResponse] = Field(description="Agent列表")

# === 工具执行请求模型 ===
class SimpleToolExecutionRequest(BaseModel):
    """简化的工具执行请求模型（用于API）"""
    tool_name: str = Field(..., description="工具名称")
    args: Dict[str, Any] = Field(default_factory=dict, description="工具参数")
    service_name: Optional[str] = Field(None, description="服务名称（可选，会自动推断）")

# === 生命周期配置模型 ===
class ServiceLifecycleConfig(BaseModel):
    """服务生命周期配置模型"""
    # 状态转换阈值
    warning_failure_threshold: Optional[int] = Field(default=None, ge=1, le=10, description="进入WARNING状态的失败阈值，范围1-10")
    reconnecting_failure_threshold: Optional[int] = Field(default=None, ge=2, le=10, description="进入RECONNECTING状态的失败阈值，范围2-10")
    max_reconnect_attempts: Optional[int] = Field(default=None, ge=3, le=20, description="最大重连尝试次数，范围3-20")

    # 重试间隔配置
    base_reconnect_delay: Optional[float] = Field(default=None, ge=0.5, le=10.0, description="基础重连延迟（秒），范围0.5-10.0")
    max_reconnect_delay: Optional[float] = Field(default=None, ge=10.0, le=300.0, description="最大重连延迟（秒），范围10.0-300.0")
    long_retry_interval: Optional[float] = Field(default=None, ge=60.0, le=1800.0, description="长周期重试间隔（秒），范围60.0-1800.0")

    # 心跳配置
    normal_heartbeat_interval: Optional[float] = Field(default=None, ge=10.0, le=300.0, description="正常心跳间隔（秒），范围10.0-300.0")
    warning_heartbeat_interval: Optional[float] = Field(default=None, ge=5.0, le=60.0, description="警告状态心跳间隔（秒），范围5.0-60.0")

    # 超时配置
    initialization_timeout: Optional[float] = Field(default=None, ge=5.0, le=120.0, description="初始化超时（秒），范围5.0-120.0")
    disconnection_timeout: Optional[float] = Field(default=None, ge=1.0, le=60.0, description="断连超时（秒），范围1.0-60.0")

# === 内容更新配置模型 ===
class ContentUpdateConfig(BaseModel):
    """服务内容更新配置模型"""
    # 更新间隔
    tools_update_interval: Optional[float] = Field(default=None, ge=60.0, le=3600.0, description="工具更新间隔（秒），范围60.0-3600.0")
    resources_update_interval: Optional[float] = Field(default=None, ge=60.0, le=3600.0, description="资源更新间隔（秒），范围60.0-3600.0")
    prompts_update_interval: Optional[float] = Field(default=None, ge=60.0, le=3600.0, description="提示词更新间隔（秒），范围60.0-3600.0")

    # 批量处理配置
    max_concurrent_updates: Optional[int] = Field(default=None, ge=1, le=10, description="最大并发更新数，范围1-10")
    update_timeout: Optional[float] = Field(default=None, ge=10.0, le=120.0, description="单次更新超时（秒），范围10.0-120.0")

    # 错误处理
    max_consecutive_failures: Optional[int] = Field(default=None, ge=1, le=10, description="最大连续失败次数，范围1-10")
    failure_backoff_multiplier: Optional[float] = Field(default=None, ge=1.0, le=5.0, description="失败退避倍数，范围1.0-5.0")

    # === 新增：健康状态阈值配置 ===
    healthy_response_threshold: Optional[float] = Field(default=None, ge=0.1, le=5.0, description="健康状态响应时间阈值（秒），范围0.1-5.0")
    warning_response_threshold: Optional[float] = Field(default=None, ge=0.5, le=10.0, description="警告状态响应时间阈值（秒），范围0.5-10.0")
    slow_response_threshold: Optional[float] = Field(default=None, ge=1.0, le=30.0, description="慢响应状态响应时间阈值（秒），范围1.0-30.0")

    # === 新增：智能超时调整配置 ===
    enable_adaptive_timeout: Optional[bool] = Field(default=None, description="是否启用智能超时调整")
    adaptive_timeout_multiplier: Optional[float] = Field(default=None, ge=1.5, le=5.0, description="智能超时倍数，范围1.5-5.0")
    response_time_history_size: Optional[int] = Field(default=None, ge=5, le=100, description="响应时间历史记录大小，范围5-100")
