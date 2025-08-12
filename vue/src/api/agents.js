import { apiRequest } from './request'

/**
 * Agent管理相关API - 基于for_agent接口的真实实现
 * Agent是服务集合的命名空间，通过服务操作来管理Agent
 */

// Agent管理API - 完全基于后端已实现的for_agent接口
export const agentsAPI = {
  // === 基础信息获取 ===

  // 获取Agent列表和统计摘要
  getAgentsList: () => apiRequest.get('/agents_summary'),

  // 获取指定Agent的服务列表
  getAgentServices: (agentId) => apiRequest.get(`/for_agent/${agentId}/list_services`),

  // 获取指定Agent的工具列表
  getAgentTools: (agentId) => apiRequest.get(`/for_agent/${agentId}/list_tools`),

  // 获取指定Agent的统计信息
  getAgentStats: (agentId) => apiRequest.get(`/for_agent/${agentId}/get_stats`),

  // === 服务管理 (Agent的核心功能) ===

  // 为Agent添加服务 (也可用于创建新Agent)
  addService: (agentId, serviceConfig) => apiRequest.post(`/for_agent/${agentId}/add_service`, serviceConfig),

  // 删除Agent的服务
  deleteService: (agentId, serviceName) => apiRequest.delete(`/for_agent/${agentId}/delete_service/${serviceName}`),

  // 更新Agent的服务配置
  updateService: (agentId, serviceName, config) => apiRequest.put(`/for_agent/${agentId}/update_service/${serviceName}`, config),

  // 获取Agent的服务详细信息
  getServiceInfo: (agentId, serviceName) => apiRequest.post(`/for_agent/${agentId}/get_service_info`, {
    name: serviceName
  }),

  // 重启Agent的服务
  restartService: (agentId, serviceName) => apiRequest.post(`/for_agent/${agentId}/restart_service`, {
    name: serviceName
  }),

  // === 生命周期状态管理 ===
  // 获取Agent服务的生命周期状态
  getServiceLifecycleStatus: (agentId, serviceName) => {
    const params = { agent_id: agentId }
    return apiRequest.get(`/health/service/${serviceName}`, { params })
  },

  // 优雅断连Agent的服务
  gracefulDisconnectService: (agentId, serviceName, reason = 'user_requested') => {
    const params = { agent_id: agentId, reason }
    return apiRequest.post(`/lifecycle/disconnect/${serviceName}`, {}, { params })
  },

  // 获取Agent服务的内容快照
  getServiceContentSnapshot: (agentId, serviceName) => {
    const params = { agent_id: agentId }
    return apiRequest.get(`/content/snapshot/${serviceName}`, { params })
  },

  // 手动刷新Agent服务的内容
  refreshServiceContent: (agentId, serviceName) => {
    const params = { agent_id: agentId }
    return apiRequest.post(`/tools/refresh/${serviceName}`, {}, { params })
  },

  // === 工具执行 ===

  // Agent使用工具
  useTool: (agentId, toolName, args) => apiRequest.post(`/for_agent/${agentId}/use_tool`, {
    tool_name: toolName,
    args
  }),

  // === 健康检查和监控 ===

  // Agent健康检查
  checkServices: (agentId) => apiRequest.get(`/for_agent/${agentId}/check_services`),

  // === 批量操作 ===

  // 批量添加服务
  batchAddServices: (agentId, services) => apiRequest.post(`/for_agent/${agentId}/batch_add_services`, {
    services
  }),

  // 批量删除服务
  batchDeleteServices: (agentId, serviceNames) => apiRequest.post(`/for_agent/${agentId}/batch_delete_services`, {
    service_names: serviceNames
  }),

  // 批量更新服务
  batchUpdateServices: (agentId, updates) => apiRequest.post(`/for_agent/${agentId}/batch_update_services`, {
    updates
  }),

  // === Agent配置重置 ===

  // 重置Agent配置 (删除所有服务)
  resetConfig: (agentId) => apiRequest.post(`/for_agent/${agentId}/reset_config`)
}

// Agent服务配置模板 - 用于添加服务时的快速配置
export const serviceTemplates = {
  // 远程HTTP服务模板
  remote: {
    name: '',
    url: '',
    transport: 'streamable-http',
    description: '远程MCP服务'
  },

  // 本地命令服务模板
  local: {
    name: '',
    command: '',
    args: [],
    env: {},
    working_dir: '',
    description: '本地MCP服务'
  }
}

// Agent服务验证函数
export const validateService = (serviceData) => {
  const errors = []

  if (!serviceData.name || serviceData.name.trim() === '') {
    errors.push('服务名称不能为空')
  }

  if (serviceData.name && !/^[a-zA-Z0-9_-]+$/.test(serviceData.name)) {
    errors.push('服务名称只能包含字母、数字、下划线和连字符')
  }

  // 远程服务验证
  if (serviceData.url) {
    try {
      new URL(serviceData.url)
    } catch {
      errors.push('URL格式不正确')
    }
  }

  // 本地服务验证
  if (serviceData.command && !serviceData.command.trim()) {
    errors.push('命令不能为空')
  }

  // 必须有URL或命令其中之一
  if (!serviceData.url && !serviceData.command) {
    errors.push('必须提供URL或命令')
  }

  return {
    isValid: errors.length === 0,
    errors
  }
}

// Agent状态常量 - 基于服务健康状态
export const AGENT_STATUS = {
  ACTIVE: 'active',      // 有健康的服务
  INACTIVE: 'inactive',  // 没有服务或所有服务都不健康
  PARTIAL: 'partial',    // 部分服务健康
  ERROR: 'error',        // 服务检查出错
  LOADING: 'loading'     // 正在加载
}

// Agent状态映射
export const AGENT_STATUS_MAP = {
  [AGENT_STATUS.ACTIVE]: '活跃',
  [AGENT_STATUS.INACTIVE]: '非活跃',
  [AGENT_STATUS.PARTIAL]: '部分可用',
  [AGENT_STATUS.ERROR]: '错误',
  [AGENT_STATUS.LOADING]: '加载中'
}

// Agent状态颜色映射
export const AGENT_STATUS_COLORS = {
  [AGENT_STATUS.ACTIVE]: 'success',
  [AGENT_STATUS.INACTIVE]: 'info',
  [AGENT_STATUS.PARTIAL]: 'warning',
  [AGENT_STATUS.ERROR]: 'danger',
  [AGENT_STATUS.LOADING]: 'primary'
}
