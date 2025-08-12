import { apiRequest } from './request'

/**
 * 服务管理相关API
 */

// Store级别服务管理
export const storeServiceAPI = {
  // 获取服务列表
  getServices: () => {
    console.log('🔍 [API] 调用 getServices:', '/for_store/list_services')
    return apiRequest.get('/for_store/list_services')
  },

  // 添加服务
  addService: (serviceConfig) => {
    console.log('🔍 [API] 调用 addService:', '/for_store/add_service', serviceConfig)
    return apiRequest.post('/for_store/add_service', serviceConfig)
  },

  // 🔧 新增：激活配置中的服务
  activateService: (serviceName) => {
    console.log('🔍 [API] 调用 activateService:', '/services/activate', { name: serviceName })
    return apiRequest.post('/services/activate', { name: serviceName })
  },

  // 获取工具列表
  getTools: () => {
    console.log('🔍 [API] 调用 getTools:', '/for_store/list_tools')
    return apiRequest.get('/for_store/list_tools')
  },

  // 使用工具
  useTool: (toolName, args) => {
    console.log('🔍 [API] 调用 useTool:', '/for_store/use_tool', { tool_name: toolName, args })
    return apiRequest.post('/for_store/use_tool', {
      tool_name: toolName,
      args
    })
  },

  // === 健康检查和状态管理 ===
  // 健康检查（兼容旧接口）
  checkServices: () => {
    console.log('🔍 [API] 调用 checkServices:', '/for_store/check_services')
    return apiRequest.get('/for_store/check_services')
  },

  // 获取生命周期状态汇总
  getLifecycleStatusSummary: () => {
    console.log('🔍 [API] 调用 getLifecycleStatusSummary:', '/health/summary')
    return apiRequest.get('/health/summary')
  },

  // 获取单个服务生命周期状态
  getServiceLifecycleStatus: (serviceName, agentId = null) => {
    const params = agentId ? { agent_id: agentId } : {}
    console.log('🔍 [API] 调用 getServiceLifecycleStatus:', `/health/service/${serviceName}`, params)
    return apiRequest.get(`/health/service/${serviceName}`, { params })
  },

  // 手动触发服务健康检查
  triggerHealthCheck: (serviceName) => {
    console.log('🔍 [API] 调用 triggerHealthCheck:', `/health/check/${serviceName}`)
    return apiRequest.post(`/health/check/${serviceName}`)
  },

  // 获取服务信息
  getServiceInfo: (serviceName) => apiRequest.post('/for_store/get_service_info', {
    name: serviceName
  }),

  // 获取服务状态（兼容旧接口）
  getServiceStatus: (serviceName) => apiRequest.post('/for_store/get_service_status', {
    name: serviceName
  }),

  // === 生命周期管理 ===
  // 优雅断连服务
  gracefulDisconnectService: (serviceName, agentId = null, reason = 'user_requested') => {
    const params = { reason }
    if (agentId) params.agent_id = agentId
    console.log('🔍 [API] 调用 gracefulDisconnectService:', `/lifecycle/disconnect/${serviceName}`, params)
    return apiRequest.post(`/lifecycle/disconnect/${serviceName}`, {}, { params })
  },

  // === 内容管理 ===
  // 获取服务内容快照
  getServiceContentSnapshot: (serviceName, agentId = null) => {
    const params = agentId ? { agent_id: agentId } : {}
    console.log('🔍 [API] 调用 getServiceContentSnapshot:', `/content/snapshot/${serviceName}`, params)
    return apiRequest.get(`/content/snapshot/${serviceName}`, { params })
  },

  // 手动刷新服务内容
  refreshServiceContent: (serviceName, agentId = null) => {
    const params = agentId ? { agent_id: agentId } : {}
    console.log('🔍 [API] 调用 refreshServiceContent:', `/tools/refresh/${serviceName}`, params)
    return apiRequest.post(`/tools/refresh/${serviceName}`, {}, { params })
  },
  
  // 重启服务
  restartService: (serviceName) => apiRequest.post('/for_store/restart_service', {
    name: serviceName
  }),
  
  // 删除服务
  deleteService: (serviceName) => apiRequest.post('/for_store/delete_service', {
    name: serviceName
  }),
  
  // 批量添加服务
  batchAddServices: (services) => apiRequest.post('/for_store/batch_add_services', {
    services
  }),
  
  // 更新服务配置（完全替换）
  updateService: (serviceName, config) => apiRequest.post('/for_store/update_service', {
    name: serviceName,
    config
  }),

  // 增量更新服务配置（推荐）
  patchService: (serviceName, updates) => apiRequest.post('/for_store/patch_service', {
    name: serviceName,
    updates
  }),

  // 批量更新服务
  batchUpdateServices: (updates) => apiRequest.post('/for_store/batch_update_services', {
    updates
  }),

  // 批量删除服务
  batchDeleteServices: (serviceNames) => apiRequest.post('/for_store/batch_delete_services', {
    service_names: serviceNames
  }),

  // 批量重启服务
  batchRestartServices: (serviceNames) => apiRequest.post('/for_store/batch_restart_services', {
    service_names: serviceNames
  }),

  // === 重置功能 ===
  // 配置链式重置 - 支持scope参数
  resetConfig: (scope = null) => {
    const url = scope ? `/for_store/reset_config?scope=${scope}` : '/for_store/reset_config'
    return apiRequest.post(url)
  },

  // 文件直接重置
  resetMcpJsonFile: () => apiRequest.post('/for_store/reset_mcp_json_file'),
  resetClientServicesFile: () => apiRequest.post('/for_store/reset_client_services_file'),
  resetAgentClientsFile: () => apiRequest.post('/for_store/reset_agent_clients_file'),

  // 获取统计信息
  getStats: () => apiRequest.get('/for_store/get_stats'),

  // 获取配置 - 支持新的show_config接口
  getConfig: () => apiRequest.get('/for_store/show_mcpconfig'),

  // 新的配置查询接口
  showConfig: (scope = 'all') => apiRequest.get(`/for_store/show_config?scope=${scope}`),

  // 更新配置
  updateConfig: (config) => apiRequest.post('/for_store/update_config', {
    config
  }),

  // 新的配置更新接口
  updateConfigNew: (serviceNameOrClientId, config) =>
    apiRequest.put(`/for_store/update_config/${serviceNameOrClientId}`, config),

  // 新的配置删除接口
  deleteConfig: (serviceNameOrClientId) =>
    apiRequest.delete(`/for_store/delete_config/${serviceNameOrClientId}`),

  // === 两步操作接口（推荐使用） ===

  // 两步操作：更新MCP JSON文件 + 重新注册服务
  updateConfigTwoStep: (config) => apiRequest.post('/for_store/update_config_two_step', {
    config
  }),

  // 两步操作：从MCP JSON文件删除服务 + 注销服务
  deleteServiceTwoStep: (serviceName) => apiRequest.post('/for_store/delete_service_two_step', {
    service_name: serviceName
  })
}

// Agent级别服务管理
export const agentServiceAPI = {
  // 获取Agent服务列表
  getServices: (agentId) => apiRequest.get(`/for_agent/${agentId}/list_services`),
  
  // 为Agent添加服务
  addService: (agentId, serviceConfig) => apiRequest.post(`/for_agent/${agentId}/add_service`, serviceConfig),
  
  // 获取Agent工具列表
  getTools: (agentId) => apiRequest.get(`/for_agent/${agentId}/list_tools`),
  
  // Agent使用工具
  useTool: (agentId, toolName, args) => apiRequest.post(`/for_agent/${agentId}/use_tool`, {
    tool_name: toolName,
    args
  }),
  
  // Agent健康检查
  checkServices: (agentId) => apiRequest.get(`/for_agent/${agentId}/check_services`),
  
  // 获取Agent服务信息
  getServiceInfo: (agentId, serviceName) => apiRequest.post(`/for_agent/${agentId}/get_service_info`, {
    name: serviceName
  }),
  
  // 获取Agent服务状态
  getServiceStatus: (agentId, serviceName) => apiRequest.post(`/for_agent/${agentId}/get_service_status`, {
    name: serviceName
  }),
  
  // 重启Agent服务
  restartService: (agentId, serviceName) => apiRequest.post(`/for_agent/${agentId}/restart_service`, {
    name: serviceName
  }),
  
  // 删除Agent服务
  deleteService: (agentId, serviceName) => apiRequest.post(`/for_agent/${agentId}/delete_service`, {
    name: serviceName
  }),

  // 更新Agent服务配置（完全替换）
  updateService: (agentId, serviceName, config) => apiRequest.post(`/for_agent/${agentId}/update_service`, {
    name: serviceName,
    config
  }),

  // 增量更新Agent服务配置（推荐）
  patchService: (agentId, serviceName, updates) => apiRequest.post(`/for_agent/${agentId}/patch_service`, {
    name: serviceName,
    updates
  }),

  // 批量更新Agent服务
  batchUpdateServices: (agentId, updates) => apiRequest.post(`/for_agent/${agentId}/batch_update_services`, {
    updates
  }),

  // 批量删除Agent服务
  batchDeleteServices: (agentId, serviceNames) => apiRequest.post(`/for_agent/${agentId}/batch_delete_services`, {
    service_names: serviceNames
  }),

  // 批量重启Agent服务
  batchRestartServices: (agentId, serviceNames) => apiRequest.post(`/for_agent/${agentId}/batch_restart_services`, {
    service_names: serviceNames
  }),

  // === Agent重置功能 ===
  // Agent配置链式重置
  resetConfig: (agentId) => apiRequest.post(`/for_agent/${agentId}/reset_config`),

  // 获取Agent统计信息
  getStats: (agentId) => apiRequest.get(`/for_agent/${agentId}/get_stats`),

  // Agent健康检查
  checkServices: (agentId) => apiRequest.get(`/for_agent/${agentId}/check_services`),

  // === 新的Agent配置管理接口 ===
  // 获取Agent配置
  showConfig: (agentId) => apiRequest.get(`/for_agent/${agentId}/show_config`),

  // 更新Agent服务配置
  updateConfigNew: (agentId, serviceNameOrClientId, config) =>
    apiRequest.put(`/for_agent/${agentId}/update_config/${serviceNameOrClientId}`, config),

  // 删除Agent服务配置
  deleteConfig: (agentId, serviceNameOrClientId) =>
    apiRequest.delete(`/for_agent/${agentId}/delete_config/${serviceNameOrClientId}`)
}

// 通用服务API
export const commonServiceAPI = {
  // 获取服务信息
  getServiceInfo: (serviceName) => apiRequest.get(`/services/${serviceName}`),
  
  // 获取所有服务概览
  getServicesOverview: () => apiRequest.get('/services/overview'),
  
  // 搜索服务
  searchServices: (query) => apiRequest.get('/services/search', { q: query }),
  
  // 获取服务统计
  getServiceStats: () => apiRequest.get('/services/stats')
}

// 本地服务管理API
export const localServiceAPI = {
  // 获取本地服务列表
  getLocalServices: () => apiRequest.get('/local_services/list'),
  
  // 启动本地服务
  startLocalService: (serviceName) => apiRequest.post('/local_services/start', {
    name: serviceName
  }),
  
  // 停止本地服务
  stopLocalService: (serviceName) => apiRequest.post('/local_services/stop', {
    name: serviceName
  }),
  
  // 重启本地服务
  restartLocalService: (serviceName) => apiRequest.post('/local_services/restart', {
    name: serviceName
  }),
  
  // 获取本地服务日志
  getLocalServiceLogs: (serviceName, lines = 100) => apiRequest.get(`/local_services/${serviceName}/logs`, {
    lines
  }),
  
  // 获取本地服务状态
  getLocalServiceStatus: (serviceName) => apiRequest.get(`/local_services/${serviceName}/status`)
}

// 服务配置模板
export const serviceTemplates = {
  // 远程HTTP服务模板
  remoteHttp: {
    name: '',
    url: '',
    transport: 'streamable-http',
    headers: {},
    env: {}
  },
  
  // 远程SSE服务模板
  remoteSSE: {
    name: '',
    url: '',
    transport: 'sse',
    headers: {},
    env: {}
  },
  
  // 本地Python服务模板
  localPython: {
    name: '',
    command: 'python',
    args: [],
    env: {},
    working_dir: ''
  },
  
  // 本地Node.js服务模板
  localNode: {
    name: '',
    command: 'node',
    args: [],
    env: {},
    working_dir: ''
  },
  
  // mcpServers格式模板
  mcpServers: {
    mcpServers: {}
  }
}

// 服务验证函数
export const validateService = (service) => {
  const errors = []
  
  if (!service.name || service.name.trim() === '') {
    errors.push('服务名称不能为空')
  }
  
  if (service.url && service.command) {
    errors.push('不能同时指定URL和命令')
  }
  
  if (!service.url && !service.command) {
    errors.push('必须指定URL或命令')
  }
  
  if (service.url && !service.url.startsWith('http')) {
    errors.push('URL必须以http或https开头')
  }
  
  if (service.command && (!service.args || !Array.isArray(service.args))) {
    errors.push('命令参数必须是数组')
  }
  
  return {
    isValid: errors.length === 0,
    errors
  }
}

// === 监控和统计API ===

// Store级别监控API
export const storeMonitoringAPI = {
  // 获取工具执行记录（替换原有的工具使用统计）
  getToolRecords: (limit = 50) => apiRequest.get('/for_store/tool_records', { params: { limit } }),

  // 检查网络端点
  checkNetworkEndpoints: (endpoints) => apiRequest.post('/for_store/network_check', { endpoints }),

  // 获取系统资源信息
  getSystemResources: () => apiRequest.get('/for_store/system_resources')
}

// Agent级别监控API
export const agentMonitoringAPI = {
  // 获取工具执行记录（替换原有的工具使用统计）
  getToolRecords: (agentId, limit = 50) => apiRequest.get(`/for_agent/${agentId}/tool_records`, { params: { limit } })
}
