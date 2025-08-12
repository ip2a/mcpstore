import { apiRequest } from './request'

/**
 * 系统管理相关API
 */

// 系统信息API
export const systemAPI = {
  // 获取系统信息
  getSystemInfo: () => apiRequest.get('/system/info'),
  
  // 获取系统配置
  getSystemConfig: () => apiRequest.get('/for_store/show_mcpconfig'),
  
  // 更新系统配置
  updateSystemConfig: (config) => apiRequest.post('/for_store/update_config', {
    config
  }),
  
  // 重置系统
  resetSystem: (type) => apiRequest.post('/system/reset', { type }),
  
  // 备份系统
  backupSystem: () => apiRequest.post('/system/backup'),
  
  // 恢复系统
  restoreSystem: (backupFile) => apiRequest.post('/system/restore', {
    backup_file: backupFile
  }),
  
  // 重启API服务
  restartAPI: () => apiRequest.post('/system/restart'),
  
  // 获取系统状态
  getSystemStatus: () => apiRequest.get('/system/status'),
  
  // 获取版本信息
  getVersionInfo: () => apiRequest.get('/system/version')
}

// 重置管理API
export const resetAPI = {
  // Store配置重置
  resetStoreConfig: () => apiRequest.post('/for_store/reset_config'),
  
  // Agent配置重置
  resetAgentConfig: (agentId) => apiRequest.post(`/for_agent/${agentId}/reset_config`),
  
  // 文件直接重置
  resetMcpJsonFile: () => apiRequest.post('/for_store/reset_mcp_json_file'),
  resetClientServicesFile: () => apiRequest.post('/for_store/reset_client_services_file'),
  resetAgentClientsFile: () => apiRequest.post('/for_store/reset_agent_clients_file'),
  
  // 批量重置
  resetAll: () => apiRequest.post('/system/reset/all'),
  
  // 重置特定类型
  resetByType: (resetType) => apiRequest.post('/system/reset', {
    type: resetType
  })
}

// 配置管理API
export const configAPI = {
  // 获取配置
  getConfig: (configType = 'mcp') => apiRequest.get('/config', {
    params: { type: configType }
  }),
  
  // 更新配置
  updateConfig: (configType, config) => apiRequest.put('/config', {
    type: configType,
    config
  }),
  
  // 验证配置
  validateConfig: (configType, config) => apiRequest.post('/config/validate', {
    type: configType,
    config
  }),
  
  // 导出配置
  exportConfig: (configType) => apiRequest.get('/config/export', {
    params: { type: configType }
  }),
  
  // 导入配置
  importConfig: (configType, configData) => apiRequest.post('/config/import', {
    type: configType,
    data: configData
  })
}

// 系统设置API
export const settingsAPI = {
  // 获取用户设置
  getUserSettings: () => apiRequest.get('/settings/user'),
  
  // 更新用户设置
  updateUserSettings: (settings) => apiRequest.put('/settings/user', settings),
  
  // 获取系统设置
  getSystemSettings: () => apiRequest.get('/settings/system'),
  
  // 更新系统设置
  updateSystemSettings: (settings) => apiRequest.put('/settings/system', settings),
  
  // 重置设置
  resetSettings: (settingsType = 'user') => apiRequest.post('/settings/reset', {
    type: settingsType
  })
}

// 系统常量
export const SYSTEM_STATUS = {
  RUNNING: 'running',
  STOPPED: 'stopped',
  STARTING: 'starting',
  STOPPING: 'stopping',
  ERROR: 'error'
}

export const RESET_TYPES = {
  STORE_CONFIG: 'store_config',
  AGENT_CONFIG: 'agent_config',
  MCP_JSON: 'mcp_json',
  CLIENT_SERVICES: 'client_services',
  AGENT_CLIENTS: 'agent_clients',
  ALL: 'all'
}

export const CONFIG_TYPES = {
  MCP: 'mcp',
  SYSTEM: 'system',
  USER: 'user',
  AGENT: 'agent'
}

// 系统信息格式化
export const formatSystemInfo = (info) => {
  return {
    version: info.version || 'Unknown',
    pythonVersion: info.python_version || 'Unknown',
    fastmcpVersion: info.fastmcp_version || 'Unknown',
    platform: info.platform || 'Unknown',
    architecture: info.architecture || 'Unknown',
    uptime: info.uptime || 0,
    startTime: info.start_time ? new Date(info.start_time) : null
  }
}

// 配置验证函数
export const validateSystemConfig = (config) => {
  const errors = []
  
  if (!config) {
    errors.push('配置不能为空')
    return { isValid: false, errors }
  }
  
  // 验证基本结构
  if (typeof config !== 'object') {
    errors.push('配置必须是对象类型')
  }
  
  // 验证必需字段
  const requiredFields = ['mcpServers']
  for (const field of requiredFields) {
    if (!config.hasOwnProperty(field)) {
      errors.push(`缺少必需字段: ${field}`)
    }
  }
  
  // 验证mcpServers结构
  if (config.mcpServers && typeof config.mcpServers !== 'object') {
    errors.push('mcpServers必须是对象类型')
  }
  
  return {
    isValid: errors.length === 0,
    errors
  }
}

// 设置默认值
export const getDefaultSystemSettings = () => ({
  theme: 'light',
  language: 'zh-CN',
  autoRefresh: true,
  refreshInterval: 30000,
  showNotifications: true,
  logLevel: 'info',
  maxLogEntries: 1000,
  enableMonitoring: true,
  monitoringInterval: 30000
})

export const getDefaultUserSettings = () => ({
  dashboardLayout: 'default',
  tablePageSize: 20,
  showAdvancedFeatures: false,
  enableKeyboardShortcuts: true,
  compactMode: false
})
