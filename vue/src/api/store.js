import { API_ENDPOINTS } from './config'
import { formatApiPath, extractResponseData } from './utils'
import { apiRequest } from './request'

/**
 * Store 级别 API 服务
 * 对应 MCPStore API v1.0.0 的 Store 级别端点
 */
export const storeApi = {
  /**
   * 服务同步
   */
  syncServices: () => apiRequest.post(API_ENDPOINTS.STORE.SYNC_SERVICES),

  syncStatus: () => apiRequest.get(API_ENDPOINTS.STORE.SYNC_STATUS),

  /**
   * 服务管理
   */
  listServices: () => apiRequest.get(API_ENDPOINTS.STORE.LIST_SERVICES)
    .then(res => {
      const data = extractResponseData(res.data, { services: [] })
      return Array.isArray(data?.services) ? data.services : []
    }),

  addService: (serviceConfig) => {
    // 后端已不支持 wait 参数，且不允许空参数；前端进行校验并直传
    if (!serviceConfig || (typeof serviceConfig === 'object' && Object.keys(serviceConfig).length === 0)) {
      throw new Error('addService: 必须提供服务配置（后端不再支持空参数全量同步）')
    }
    if ('wait' in (serviceConfig || {}) || ('options' in (serviceConfig || {}) && serviceConfig.options && 'wait' in serviceConfig.options)) {
      throw new Error('addService: 后端不再支持 wait 参数，请移除后重试')
    }
    return apiRequest.post(API_ENDPOINTS.STORE.ADD_SERVICE, serviceConfig)
  },

  initService: (serviceName) => apiRequest.post(API_ENDPOINTS.STORE.INIT_SERVICE, { name: serviceName }),

  deleteService: (serviceName) => apiRequest.delete(
    formatApiPath(API_ENDPOINTS.STORE.DELETE_SERVICE, { service_name: serviceName })
  ),

  /**
   * 工具管理
   */
  listTools: () => apiRequest.get(API_ENDPOINTS.STORE.LIST_TOOLS)
    .then(res => {
      const data = extractResponseData(res.data, { tools: [] })
      return Array.isArray(data?.tools) ? data.tools : []
    }),

  getTools: () => apiRequest.get(API_ENDPOINTS.STORE.LIST_TOOLS)
    .then(res => {
      const data = extractResponseData(res.data, { tools: [] })
      return Array.isArray(data?.tools) ? data.tools : []
    }),

  getToolInfo: (toolName) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.STORE.TOOL_INFO, { tool_name: toolName })
  ).then(res => extractResponseData(res.data)),

  callTool: (toolName, args, config = {}) => apiRequest.post(API_ENDPOINTS.STORE.CALL_TOOL, {
    tool_name: toolName,
    args: args || {}
  }, config),

  /**
   * 服务详情
   */
  getServiceInfo: (serviceName) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.STORE.SERVICE_INFO, { service_name: serviceName })
  ).then(res => extractResponseData(res.data)),

  getServiceStatus: (serviceName) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.STORE.SERVICE_STATUS, { service_name: serviceName })
  ).then(res => extractResponseData(res.data)),

  /**
   * 健康检查
   */
  checkServiceHealth: (serviceName) => apiRequest.post(
    formatApiPath(API_ENDPOINTS.STORE.SERVICE_HEALTH, { service_name: serviceName })
  ).then(res => extractResponseData(res.data)),

  checkServices: () => apiRequest.get(API_ENDPOINTS.STORE.CHECK_SERVICES)
    .then(res => extractResponseData(res.data)),

  getHealth: () => apiRequest.get(API_ENDPOINTS.STORE.HEALTH)
    .then(res => extractResponseData(res.data)),

  /**
   * 配置管理
   */
  getConfig: (scope = 'all') => apiRequest.get(API_ENDPOINTS.STORE.SHOW_CONFIG, { params: { scope } })
    .then(res => extractResponseData(res.data)),

  updateConfig: (clientIdOrServiceName, config) => apiRequest.put(
    formatApiPath(API_ENDPOINTS.STORE.UPDATE_CONFIG, { client_id_or_service_name: clientIdOrServiceName }),
    config
  ),

  resetConfig: (scope = 'all') => apiRequest.post(API_ENDPOINTS.STORE.RESET_CONFIG, { scope }),

  /**
   * MCP JSON 配置管理
   */
  getMcpJson: () => apiRequest.get(API_ENDPOINTS.STORE.SHOW_MCPJSON)
    .then(res => extractResponseData(res.data)),

  resetMcpJson: (config) => apiRequest.post(API_ENDPOINTS.STORE.RESET_MCPJSON, config),

  /**
   * 统计信息 - 使用 /health 接口（后端没有 get_stats 接口）
   */
  getStats: () => apiRequest.get(API_ENDPOINTS.STORE.HEALTH)
    .then(res => {
      const health = extractResponseData(res.data, {})
      // 转换为前端需要的格式
      return {
        services_count: health.services_count || 0,
        agents_count: health.agents_count || 0,
        status: health.status || 'unknown',
        uptime_seconds: health.uptime_seconds || 0
      }
    }),


  /**
   * 系统资源
   */
  getSystemResources: () => apiRequest.get(API_ENDPOINTS.STORE.SYSTEM_RESOURCES)
    .then(res => extractResponseData(res.data)),

  checkNetwork: (endpoints) => apiRequest.post(API_ENDPOINTS.STORE.NETWORK_CHECK, { endpoints }),

  /**
   * Agent 管理
   */
  listAllAgents: () => apiRequest.get(API_ENDPOINTS.STORE.LIST_ALL_AGENTS)
    .then(res => extractResponseData(res.data, [])),

  listServicesByAgent: (agentId) => apiRequest.get(API_ENDPOINTS.STORE.LIST_SERVICES_BY_AGENT, {
    params: { agent_id: agentId }
  }).then(res => extractResponseData(res.data, [])),

  /**
   * 服务重启
   */
  restartService: (serviceName) => apiRequest.post(API_ENDPOINTS.STORE.RESTART_SERVICE, {
    service_name: serviceName
  }),

  /**
   * 服务更新
   */
  patchService: (serviceName, updates) => apiRequest.patch(
    formatApiPath(API_ENDPOINTS.STORE.UPDATE_SERVICE, { service_name: serviceName }),
    updates
  ),

  updateService: (serviceName, updates) => apiRequest.patch(
    formatApiPath(API_ENDPOINTS.STORE.UPDATE_SERVICE, { service_name: serviceName }),
    updates
  ),

  /**
   * 批量操作
   */
  batchUpdateServices: (serviceNames, updates) => apiRequest.patch(
    API_ENDPOINTS.STORE.BATCH_UPDATE_SERVICES,
    { service_names: serviceNames, updates }
  ),

  batchDeleteServices: (serviceNames) => apiRequest.post(API_ENDPOINTS.STORE.BATCH_DELETE_SERVICES, {
    service_names: serviceNames
  }),

  batchRestartServices: (serviceNames) => apiRequest.post(API_ENDPOINTS.STORE.BATCH_RESTART_SERVICES, {
    service_names: serviceNames
  })
}
