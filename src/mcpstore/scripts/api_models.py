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

# === 服务详情相关响应模型 ===

class ServiceLifecycleInfo(BaseModel):
    """服务生命周期信息"""
    consecutive_successes: int = Field(description="连续成功次数")
    consecutive_failures: int = Field(description="连续失败次数")
    last_ping_time: Optional[str] = Field(None, description="最后ping时间")
    error_message: Optional[str] = Field(None, description="错误信息")
    reconnect_attempts: int = Field(description="重连尝试次数")
    state_entered_time: Optional[str] = Field(None, description="状态进入时间")

class ServiceToolInfo(BaseModel):
    """服务工具信息"""
    name: str = Field(description="工具名称")
    description: Optional[str] = Field(None, description="工具描述")
    input_schema: Optional[Dict[str, Any]] = Field(None, description="输入模式")
    service_name: str = Field(description="所属服务名称")

class ServiceHealthDetail(BaseModel):
    """服务健康详情"""
    status: str = Field(description="健康状态")
    message: Optional[str] = Field(None, description="健康消息")
    timestamp: Optional[str] = Field(None, description="检查时间戳")
    uptime: Optional[str] = Field(None, description="运行时间")
    error_count: int = Field(default=0, description="错误计数")
    last_error: Optional[str] = Field(None, description="最后错误")
    response_time: Optional[float] = Field(None, description="响应时间（毫秒）")
    is_healthy: bool = Field(description="是否健康")

class ServiceDetailResponse(BaseModel):
    """服务详细信息响应"""
    name: str = Field(description="服务名称")
    status: str = Field(description="服务状态")
    transport: str = Field(description="传输类型")
    client_id: Optional[str] = Field(None, description="客户端ID")
    url: Optional[str] = Field(None, description="服务URL")
    command: Optional[str] = Field(None, description="启动命令")
    args: Optional[List[str]] = Field(None, description="命令参数")
    env: Optional[Dict[str, str]] = Field(None, description="环境变量")
    tool_count: int = Field(description="工具数量")
    is_active: bool = Field(description="是否已激活")
    config: Dict[str, Any] = Field(default_factory=dict, description="配置信息")
    lifecycle: Optional[ServiceLifecycleInfo] = Field(None, description="生命周期信息")
    tools: List[ServiceToolInfo] = Field(default_factory=list, description="工具列表")
    health: Optional[ServiceHealthDetail] = Field(None, description="健康信息")

class ServiceStatusResponse(BaseModel):
    """服务状态响应"""
    name: str = Field(description="服务名称")
    status: str = Field(description="服务状态")
    is_active: bool = Field(description="是否已激活")
    client_id: Optional[str] = Field(None, description="客户端ID")
    last_updated: Optional[str] = Field(None, description="最后更新时间")
    consecutive_successes: int = Field(default=0, description="连续成功次数")
    consecutive_failures: int = Field(default=0, description="连续失败次数")
    error_message: Optional[str] = Field(None, description="错误信息")
    reconnect_attempts: int = Field(default=0, description="重连尝试次数")

# === 数据空间相关响应模型 ===

class WorkspaceInfo(BaseModel):
    """工作空间信息"""
    name: str = Field(description="工作空间名称")
    path: str = Field(description="工作空间路径")
    mcp_config_path: str = Field(description="MCP配置文件路径")
    is_current: bool = Field(description="是否为当前工作空间")

class DataSpaceInfo(BaseModel):
    """数据空间信息"""
    is_using_data_space: bool = Field(description="是否使用数据空间")
    workspace_dir: Optional[str] = Field(None, description="工作空间目录")
    mcp_config_path: Optional[str] = Field(None, description="MCP配置文件路径")
    data_space_path: Optional[str] = Field(None, description="数据空间路径")
    workspace_config: Dict[str, Any] = Field(default_factory=dict, description="工作空间配置")

class WorkspacesListResponse(BaseModel):
    """工作空间列表响应"""
    workspaces: List[WorkspaceInfo] = Field(description="工作空间列表")
    current_workspace: Optional[str] = Field(None, description="当前工作空间路径")
    using_default: bool = Field(default=False, description="是否使用默认配置")

# === LangChain 相关响应模型 ===

class LangChainToolParameter(BaseModel):
    """LangChain工具参数信息"""
    required: List[str] = Field(default_factory=list, description="必需参数")
    optional: List[str] = Field(default_factory=list, description="可选参数")
    total_count: int = Field(default=0, description="参数总数")

class LangChainToolResponse(BaseModel):
    """LangChain工具响应"""
    name: str = Field(description="工具名称")
    description: str = Field(description="工具描述")
    args_schema: Optional[Dict[str, Any]] = Field(None, description="参数模式")
    is_structured: bool = Field(description="是否为结构化工具")
    tool_type: str = Field(description="工具类型")
    parameters: Optional[LangChainToolParameter] = Field(None, description="参数信息")
    original_info: Optional[Dict[str, Any]] = Field(None, description="原始工具信息")

class LangChainToolsListResponse(BaseModel):
    """LangChain工具列表响应"""
    tools: List[LangChainToolResponse] = Field(description="工具列表")
    total_tools: int = Field(description="工具总数")
    structured_tools: int = Field(description="结构化工具数量")

# === 批量操作请求模型 ===

class BatchServiceOperationRequest(BaseModel):
    """批量服务操作请求"""
    service_names: List[str] = Field(..., description="服务名称列表")
    operation: str = Field(..., description="操作类型: init, start, stop, restart, delete")

class BatchServiceOperationResponse(BaseModel):
    """批量服务操作响应"""
    total_count: int = Field(description="总数")
    success_count: int = Field(description="成功数量")
    failure_count: int = Field(description="失败数量")
    results: List[Dict[str, Any]] = Field(description="各服务操作结果")

# === API分页模型 ===

class PaginationParams(BaseModel):
    """分页参数"""
    page: int = Field(default=1, ge=1, description="页码")
    page_size: int = Field(default=20, ge=1, le=100, description="每页大小")

class PaginatedResponse(BaseModel):
    """分页响应基类"""
    items: List[Any] = Field(description="数据项")
    total: int = Field(description="总数")
    page: int = Field(description="当前页码")
    page_size: int = Field(description="每页大小")
    total_pages: int = Field(description="总页数")

# === 生命周期配置扩展 ===

class ExtendedServiceLifecycleConfig(ServiceLifecycleConfig):
    """扩展的服务生命周期配置模型"""
    # 重试间隔配置
    base_reconnect_delay: Optional[float] = Field(default=None, ge=0.5, le=10.0, description="基础重连延迟（秒），范围0.5-10.0")
    max_reconnect_delay: Optional[float] = Field(default=None, ge=10.0, le=300.0, description="最大重连延迟（秒），范围10.0-300.0")
    
    # 健康检查配置
    health_check_interval: Optional[float] = Field(default=None, ge=5.0, le=300.0, description="健康检查间隔（秒），范围5.0-300.0")
    health_check_timeout: Optional[float] = Field(default=None, ge=1.0, le=60.0, description="健康检查超时（秒），范围1.0-60.0")
    
    # 性能监控配置
    enable_performance_metrics: Optional[bool] = Field(default=None, description="是否启用性能指标收集")
    metrics_retention_days: Optional[int] = Field(default=None, ge=1, le=365, description="指标保留天数，范围1-365")
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
