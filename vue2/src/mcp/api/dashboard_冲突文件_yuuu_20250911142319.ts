// MCP 仪表盘数据 API
import http from '@/mcp/api/http'

// 仪表盘统计数据接口
export interface DashboardStats {
  services: {
    total: number
    healthy: number
    warning: number
    error: number
  }
  tools: {
    total: number
    executable: number
    recent_executions: number
  }
  agents: {
    total: number
    active: number
  }
  system: {
    uptime: string
    memory_usage: number
    disk_usage: number
    status: 'running' | 'stopped'
  }
}

// 工具调用记录接口
export interface ToolRecord {
  id: string
  tool_name: string
  service_name: string
  params: Record<string, any>
  result?: any
  error?: string
  response_time: number
  execution_time: string
  timestamp: number
  is_error: boolean
}

// 服务健康状态接口
export interface ServiceHealth {
  service_name: string
  status: 'initializing' | 'healthy' | 'warning' | 'reconnecting' | 'unreachable' | 'disconnected'
  response_time: number
  last_check_time: number
  consecutive_failures: number
  consecutive_successes: number
  error_message?: string
}

// 系统资源信息接口
export interface SystemResources {
  server_uptime: string
  memory_total: number
  memory_used: number
  memory_percentage: number
  disk_usage_percentage: number
  network_traffic_in: number
  network_traffic_out: number
}

// Agent 统计信息接口
export interface AgentSummary {
  total_agents: number
  active_agents: number
  total_services: number
  total_tools: number
  store_services: number
  store_tools: number
  agents: Array<{
    agent_id: string
    service_count: number
    tool_count: number
    healthy_services: number
    unhealthy_services: number
    total_tool_executions: number
    last_activity: string
  }>
}

export const dashboardApi = {
  // 获取服务列表和统计
  getServices: () => http.get<any>({ url: '/for_store/list_services' }),

  // 获取工具列表和统计
  getTools: () => http.get<any>({ url: '/for_store/list_tools' }),

  // 获取工具调用记录
  getToolRecords: (limit = 10) => http.get<any>({
    url: '/for_store/tool_records',
    params: { limit }
  }),

  // 获取系统资源信息
  getSystemResources: () => http.get<any>({
    url: '/for_store/system_resources'
  }),

  // 获取健康状态汇总
  getHealthSummary: () => http.get<any>({
    url: '/health/summary'
  }),

  // 获取 Agent 统计
  getAgentsSummary: () => http.get<any>({
    url: '/agents_summary'
  }),

  // 获取单个服务详情
  getServiceInfoByName: (serviceName: string) => http.get<any>({
    url: `/for_store/service_info/${encodeURIComponent(serviceName)}`
  }),

  // 获取服务健康检查
  checkServices: () => http.get<any>({
    url: '/for_store/check_services'
  }),

  // 同步服务
  syncServices: () => http.post<any>({
    url: '/for_store/sync_services'
  }),

  // 添加服务（支持空参数/远程/本地）
  addService: (payload?: any, wait: number | string = 'auto') =>
    http.post<any>({ url: `/for_store/add_service?wait=${wait}`, data: payload ?? null }),

  // 获取统计信息
  getStats: () => http.get<any>({
    url: '/for_store/get_stats'
  }),

  // 服务操作
  restartService: (serviceName: string) => http.post<any>({
    url: '/for_store/restart_service',
    data: { service_name: serviceName }
  }),
  deleteServiceTwoStep: (serviceName: string) => http.post<any>({
    url: '/for_store/delete_service_two_step',
    data: { service_name: serviceName }
  }),
  activateService: (serviceName: string) => http.post<any>({
    url: '/services/activate',
    data: { name: serviceName }
  }),
  disconnectService: (serviceName: string, reason = 'user_requested') => http.post<any>({
    url: `/lifecycle/disconnect/${encodeURIComponent(serviceName)}`,
    params: { reason }
  }),
  updateService: (serviceName: string, payload: any) => http.put<any>({
    url: `/for_store/update_service/${encodeURIComponent(serviceName)}`,
    data: payload
  }),

  // 配置管理
  showMcpConfig: () => http.get<any>({ url: '/for_store/show_mcpconfig' }),
  getJsonConfig: () => http.get<any>({ url: '/for_store/get_json_config' }),
  updateConfig: (clientIdOrServiceName: string, newConfig: any) => http.put<any>({
    url: `/for_store/update_config/${encodeURIComponent(clientIdOrServiceName)}`,
    data: newConfig
  }),
  resetMcpJsonFile: () => http.post<any>({ url: '/for_store/reset_mcp_json_file' }),

  // 执行工具（参考旧版API：POST /for_store/call_tool { tool_name, args }）
  callTool: (toolName: string, args?: Record<string, any>, config: any = {}) =>
    http.post<any>({ url: '/for_store/call_tool', data: { tool_name: toolName, args: args ?? {} }, ...config })
}

// 数据转换工具函数
export const transformDashboardData = {
  // 转换服务数据为统计卡片格式
  transformServices: (servicesResponse: any): { total: number, healthy: number, warning: number, error: number } => {
    // 处理 MCPStore API 响应格式
    const services = servicesResponse?.data?.services || servicesResponse?.services || []
    const total = services.length
    const healthy = services.filter((s: any) => s.status === 'healthy').length
    const warning = services.filter((s: any) => s.status === 'warning').length
    const error = services.filter((s: any) => ['unreachable', 'disconnected'].includes(s.status)).length

    return { total, healthy, warning, error }
  },
  
  // 转换工具数据为统计格式
  transformTools: (toolsResponse: any): { total: number, executable: number, recent_executions: number } => {
    // 处理 MCPStore API 响应格式
    // listtools 返回的是 { data: [...], metadata: {...} }
    const tools = toolsResponse?.data || []
    const metadata = toolsResponse?.metadata || {}

    return {
      total: metadata.total_tools || tools.length,
      executable: metadata.executable_tools || tools.filter((t: any) => t.executable !== false).length,
      recent_executions: tools.reduce((sum: number, t: any) => sum + (t.execution_count || 0), 0)
    }
  },
  
  // 转换工具记录为图表数据
  transformToolRecordsToChart: (records: ToolRecord[]): number[] => {
    // 按小时分组统计调用次数
    const hourlyStats = new Array(24).fill(0)
    const now = new Date()
    
    records.forEach(record => {
      const recordTime = new Date(record.execution_time)
      const hoursDiff = Math.floor((now.getTime() - recordTime.getTime()) / (1000 * 60 * 60))
      if (hoursDiff >= 0 && hoursDiff < 24) {
        hourlyStats[23 - hoursDiff]++
      }
    })
    
    return hourlyStats
  },
  
  // 转换系统资源数据
  transformSystemResources: (resources: SystemResources) => ({
    uptime: resources.server_uptime,
    memory_usage: resources.memory_percentage,
    disk_usage: resources.disk_usage_percentage,
    status: 'running' as const
  })
}
