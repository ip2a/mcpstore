// MCP API 适配层（对齐 API 接口文档 v2）
// MCP-only mode: 使用 MCP 专用 http 客户端；原 utils/http 将在精简完成后删除
import http from '@/mcp/api/http'

export interface McpServiceItem {
  name: string
  status: string
  type: string
  tools_count?: number
  last_check?: string
}

export interface McpToolItem {
  name: string
  service?: string
  description?: string
  input_schema?: any
}

export const mcpApi = {
  // 核心：服务 / 工具（统一解包 data 字段，便于列表直接使用）
  listServices: async () => {
    const res = await http.get<any>({ url: '/for_store/list_services' })
    return res?.data || []
  },
  listTools: async () => {
    const res = await http.get<any>({ url: '/for_store/list_tools' })
    return res?.data || []
  },

  // 服务详情
  getServiceInfo: (serviceName: string) => http.get<any>({ url: `/for_store/service_info/${encodeURIComponent(serviceName)}` }),

  // 工具执行（兼容：同时携带 args 与 arguments 字段）
  useTool: (toolName: string, args: Record<string, any> = {}, serviceName?: string) =>
    http.post<any>({
      url: '/for_store/use_tool',
      data: { tool_name: toolName, args, arguments: args, service_name: serviceName }
    }),

  // 兼容命名：旧 callTool -> 新 useTool（同上携带 args 与 arguments）
  callTool: (toolName: string, args: Record<string, any> = {}, serviceName?: string) =>
    http.post<any>({
      url: '/for_store/use_tool',
      data: { tool_name: toolName, args, arguments: args, service_name: serviceName }
    }),


  // 监控 / 列表
  checkServices: () => http.get<any>({ url: '/for_store/check_services' }),
  listAllAgents: () => http.get<any>({ url: '/for_store/list_all_agents' }),
  getToolRecords: (params: { tool_name?: string; service_name?: string; page?: number; page_size?: number } = { page: 1, page_size: 10 }) =>
    http.get<any>({ url: '/for_store/tool_records', params }),

  // 服务运维
  restartService: (serviceName: string) => http.post<any>({ url: '/for_store/restart_service', data: { service_name: serviceName } }),
  waitService: (serviceName: string, timeout_seconds = 30) => http.post<any>({ url: '/for_store/wait_service', data: { service_name: serviceName, timeout_seconds } }),
  deleteService: (serviceName: string) => http.del<any>({ url: `/for_store/delete_service/${encodeURIComponent(serviceName)}` }),

  // 配置管理（v2）
  showMcpJson: () => http.get<any>({ url: '/for_store/show_mcpjson' }),
  showConfig: () => http.get<any>({ url: '/for_store/show_config' }),
  updateConfig: (key: string, newConfig: any) => http.put<any>({ url: `/for_store/update_config/${encodeURIComponent(key)}`, data: newConfig }),
  deleteConfig: (key: string) => http.del<any>({ url: `/for_store/delete_config/${encodeURIComponent(key)}` }),
  resetConfig: () => http.post<any>({ url: '/for_store/reset_config' }),

  // 系统
  getHealth: () => http.get<any>({ url: '/health' }),
  getRootInfo: () => http.get<any>({ url: '/' })
}

