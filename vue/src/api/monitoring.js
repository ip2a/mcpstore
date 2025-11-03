import { API_ENDPOINTS } from './config'
import { formatApiPath, extractResponseData } from './utils'
import { apiRequest } from './request'

/**
 * 监控和生命周期 API 服务
 * 对应 MCPStore API v1.0.0 的监控端点
 */
export const monitoringApi = {
  /**
   * Agent 统计
   */
  getAgentsSummary: () => apiRequest.get(API_ENDPOINTS.MONITORING.AGENTS_SUMMARY)
    .then(res => extractResponseData(res.data)),
  
  /**
   * 生命周期配置
   */
  getLifecycleConfig: () => apiRequest.get(API_ENDPOINTS.MONITORING.LIFECYCLE_CONFIG)
    .then(res => extractResponseData(res.data)),
  
  updateLifecycleConfig: (config) => apiRequest.post(API_ENDPOINTS.MONITORING.LIFECYCLE_CONFIG, config),
  
  /**
   * 健康状态汇总
   */
  getHealthSummary: () => apiRequest.get(API_ENDPOINTS.MONITORING.HEALTH_SUMMARY)
    .then(res => extractResponseData(res.data)),
  
  getServiceHealth: (serviceName, agentId = null) => {
    const params = agentId ? { agent_id: agentId } : {}
    return apiRequest.get(
      formatApiPath(API_ENDPOINTS.MONITORING.HEALTH_SERVICE, { service_name: serviceName }),
      { params }
    ).then(res => extractResponseData(res.data))
  },
  
  triggerHealthCheck: (serviceName) => apiRequest.post(
    formatApiPath(API_ENDPOINTS.MONITORING.HEALTH_CHECK, { service_name: serviceName })
  ).then(res => extractResponseData(res.data)),
  
  /**
   * 内容管理
   */
  refreshAllTools: () => apiRequest.post(API_ENDPOINTS.MONITORING.TOOLS_REFRESH)
    .then(res => extractResponseData(res.data)),
  
  refreshServiceTools: (serviceName, agentId = null) => {
    const params = agentId ? { agent_id: agentId } : {}
    return apiRequest.post(
      formatApiPath(API_ENDPOINTS.MONITORING.TOOLS_REFRESH_SERVICE, { service_name: serviceName }),
      params
    ).then(res => extractResponseData(res.data))
  },
  
  getToolsUpdateStatus: () => apiRequest.get(API_ENDPOINTS.MONITORING.TOOLS_UPDATE_STATUS)
    .then(res => extractResponseData(res.data)),
  
  /**
   * 内容快照
   */
  getServiceContentSnapshot: (serviceName, agentId = null) => {
    const params = agentId ? { agent_id: agentId } : {}
    return apiRequest.get(
      formatApiPath(API_ENDPOINTS.MONITORING.CONTENT_SNAPSHOT, { service_name: serviceName }),
      { params }
    ).then(res => extractResponseData(res.data))
  },
  
  getAllContentSnapshots: () => apiRequest.get(API_ENDPOINTS.MONITORING.CONTENT_SNAPSHOTS)
    .then(res => extractResponseData(res.data)),
  
  /**
   * 生命周期管理
   */
  gracefulDisconnect: (serviceName, agentId = null, reason = 'user_requested') => {
    const params = agentId ? { agent_id: agentId } : {}
    return apiRequest.post(
      formatApiPath(API_ENDPOINTS.MONITORING.LIFECYCLE_DISCONNECT, { service_name: serviceName }),
      { reason, ...params }
    ).then(res => extractResponseData(res.data))
  },
  
  /**
   * 告警管理
   */
  addAlert: (alert) => apiRequest.post(API_ENDPOINTS.MONITORING.ALERTS, alert),
  
  getAlerts: (limit = 50) => apiRequest.get(API_ENDPOINTS.MONITORING.ALERTS, {
    params: { limit }
  }).then(res => extractResponseData(res.data, [])),
  
  clearAlerts: () => apiRequest.delete(API_ENDPOINTS.MONITORING.ALERTS),
  
  /**
   * 性能监控
   */
  getPerformanceMetrics: () => apiRequest.get(API_ENDPOINTS.MONITORING.PERFORMANCE)
    .then(res => extractResponseData(res.data)),
  
  getUsageStatistics: () => apiRequest.get(API_ENDPOINTS.MONITORING.USAGE_STATS)
    .then(res => extractResponseData(res.data))
}