/**
 * API统一导出文件
 * 按照开发文档的结构组织API接口
 */

// 基础请求封装
export { apiRequest } from './request'

// 服务管理API
export {
  storeServiceAPI,
  agentServiceAPI,
  commonServiceAPI,
  localServiceAPI,
  serviceTemplates,
  validateService,
  storeMonitoringAPI as servicesMonitoringAPI,
  agentMonitoringAPI as servicesAgentMonitoringAPI
} from './services'

// 工具管理API
export {
  storeToolsAPI,
  agentToolsAPI,
  validateToolParams,
  generateToolParamsTemplate
} from './tools'

// Agent管理API
export {
  agentsAPI,
  serviceTemplates,
  validateService,
  AGENT_STATUS,
  AGENT_STATUS_MAP,
  AGENT_STATUS_COLORS
} from './agents'

// 注意：监控API已移除，相关功能已整合到services.js中

// 系统管理API
export {
  systemAPI,
  resetAPI,
  configAPI,
  settingsAPI,
  SYSTEM_STATUS,
  RESET_TYPES,
  CONFIG_TYPES,
  formatSystemInfo,
  validateSystemConfig,
  getDefaultSystemSettings,
  getDefaultUserSettings
} from './system'

// 兼容性导出 - 保持与现有代码的兼容性
export { storeServiceAPI as servicesAPI } from './services'
export { storeToolsAPI as toolsAPI } from './tools'
export { storeMonitoringAPI as monitoringAPI } from './services'

// 统一的API对象，按功能模块组织
export const API = {
  // 服务管理
  services: {
    store: storeServiceAPI,
    agent: agentServiceAPI,
    common: commonServiceAPI,
    local: localServiceAPI
  },
  
  // 工具管理
  tools: {
    store: storeToolsAPI,
    agent: agentToolsAPI
  },
  
  // Agent管理
  agents: agentsAPI,
  
  // 监控功能已整合到services模块中
  
  // 系统管理
  system: {
    info: systemAPI,
    reset: resetAPI,
    config: configAPI,
    settings: settingsAPI
  }
}

// 默认导出
export default API
