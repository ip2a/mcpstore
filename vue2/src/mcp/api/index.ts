// MCP API 适配层（与 mcpstore/vue 对齐的端点定义与最小封装）
import http from '@/utils/http'

export const mcpApi = {
  // 核心：服务列表 / 工具列表
  listServices: () => http.get<string[]>({ url: '/for_store/list_services' }),
  listTools: () => http.get<any[]>({ url: '/for_store/list_tools' }),

  // 服务详情
  getServiceInfo: (serviceName: string) => http.get<any>({ url: `/for_store/service_info/${serviceName}` }),

  // 工具详情/调用
  getToolInfo: (toolName: string) => http.get<any>({ url: `/for_store/tool_info/${toolName}` }),
  callTool: (toolName: string, args: Record<string, any>) =>
    http.post<any>({ url: '/for_store/call_tool', data: { tool_name: toolName, args } }),

  // 统计/监控
  getStats: () => http.get<any>({ url: '/for_store/get_stats' }),
  getToolRecords: (limit = 10) => http.get<any[]>({ url: '/for_store/tool_records', params: { limit } }),
  getSystemResources: () => http.get<any>({ url: '/for_store/system_resources' }),
  checkServices: () => http.get<any[]>({ url: '/for_store/check_services' }),
  listAllAgents: () => http.get<any[]>({ url: '/for_store/list_all_agents' }),

  // 快速操作
  syncServices: () => http.post<any>({ url: '/for_store/sync_services' })
}

