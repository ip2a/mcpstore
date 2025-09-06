import { API_ENDPOINTS } from './config'
import { formatApiPath, extractResponseData } from './utils'
import { apiRequest } from './request'

/**
 * Agent 级别 API 服务
 * 对应 MCPStore API v1.0.0 的 Agent 级别端点
 */
export const agentApi = {
  /**
   * Agent 管理
   */
  getAgentsList: () => apiRequest.get(API_ENDPOINTS.STORE.LIST_ALL_AGENTS)
    .then(res => extractResponseData(res.data, [])),
  
  /**
   * 服务管理
   */
  addService: (agentId, payload) => apiRequest.post(
    formatApiPath(API_ENDPOINTS.AGENT.ADD_SERVICE, { agent_id: agentId }),
    payload
  ),
  
  listServices: (agentId) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.AGENT.LIST_SERVICES, { agent_id: agentId })
  ).then(res => extractResponseData(res.data, [])),
  
  initService: (agentId, identifier) => apiRequest.post(
    formatApiPath(API_ENDPOINTS.AGENT.INIT_SERVICE, { agent_id: agentId }),
    identifier
  ),
  
  deleteService: (agentId, serviceName) => apiRequest.delete(
    formatApiPath(API_ENDPOINTS.AGENT.DELETE_SERVICE, { 
      agent_id: agentId,
      service_name: serviceName 
    })
  ),
  
  updateService: (agentId, serviceName, config) => apiRequest.put(
    formatApiPath(API_ENDPOINTS.AGENT.UPDATE_SERVICE, { 
      agent_id: agentId,
      service_name: serviceName 
    }),
    config
  ),
  
  waitService: (agentId, params) => apiRequest.post(
    formatApiPath(API_ENDPOINTS.AGENT.WAIT_SERVICE, { agent_id: agentId }),
    params
  ),
  
  restartService: (agentId, serviceName) => apiRequest.post(
    formatApiPath(API_ENDPOINTS.AGENT.RESTART_SERVICE, { agent_id: agentId }),
    { service_name: serviceName }
  ),
  
  /**
   * 工具管理
   */
  listTools: (agentId) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.AGENT.LIST_TOOLS, { agent_id: agentId })
  ).then(res => extractResponseData(res.data, [])),
  
  callTool: (agentId, toolName, args, serviceName) => apiRequest.post(
    formatApiPath(API_ENDPOINTS.AGENT.CALL_TOOL, { agent_id: agentId }),
    {
      tool_name: toolName,
      args: args || {},
      service_name: serviceName
    }
  ),
  
  // 向后兼容
  useTool: (agentId, toolName, args, serviceName) => apiRequest.post(
    formatApiPath(API_ENDPOINTS.AGENT.USE_TOOL, { agent_id: agentId }),
    {
      tool_name: toolName,
      args: args || {},
      service_name: serviceName
    }
  ),
  
  /**
   * 服务详情
   */
  getServiceInfo: (agentId, serviceName) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.AGENT.SERVICE_INFO, { 
      agent_id: agentId,
      service_name: serviceName 
    })
  ).then(res => extractResponseData(res.data)),
  
  getServiceStatus: (agentId, serviceName) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.AGENT.SERVICE_STATUS, { 
      agent_id: agentId,
      service_name: serviceName 
    })
  ).then(res => extractResponseData(res.data)),
  
  checkServiceHealth: (agentId, serviceName) => apiRequest.post(
    formatApiPath(API_ENDPOINTS.AGENT.SERVICE_HEALTH, { 
      agent_id: agentId,
      service_name: serviceName 
    })
  ).then(res => extractResponseData(res.data)),
  
  getServiceHealthDetails: (agentId, serviceName) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.AGENT.SERVICE_HEALTH_DETAILS, { 
      agent_id: agentId,
      service_name: serviceName 
    })
  ).then(res => extractResponseData(res.data)),
  
  /**
   * 健康检查
   */
  checkServices: (agentId) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.AGENT.CHECK_SERVICES, { agent_id: agentId })
  ).then(res => extractResponseData(res.data)),
  
  getHealth: (agentId) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.AGENT.HEALTH, { agent_id: agentId })
  ).then(res => extractResponseData(res.data)),
  
  /**
   * 配置管理
   */
  showConfig: (agentId) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.AGENT.SHOW_CONFIG, { agent_id: agentId })
  ).then(res => extractResponseData(res.data)),
  
  showMcpConfig: (agentId) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.AGENT.SHOW_MCP_CONFIG, { agent_id: agentId })
  ).then(res => extractResponseData(res.data)),
  
  getJsonConfig: (agentId) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.AGENT.GET_JSON_CONFIG, { agent_id: agentId })
  ).then(res => extractResponseData(res.data)),
  
  updateConfig: (agentId, clientIdOrServiceName, config) => apiRequest.put(
    formatApiPath(API_ENDPOINTS.AGENT.UPDATE_CONFIG, { 
      agent_id: agentId,
      client_id_or_service_name: clientIdOrServiceName 
    }),
    config
  ),
  
  deleteConfig: (agentId, clientIdOrServiceName) => apiRequest.delete(
    formatApiPath(API_ENDPOINTS.AGENT.DELETE_CONFIG, { 
      agent_id: agentId,
      client_id_or_service_name: clientIdOrServiceName 
    })
  ),
  
  resetConfig: (agentId) => apiRequest.post(
    formatApiPath(API_ENDPOINTS.AGENT.RESET_CONFIG, { agent_id: agentId })
  ),
  
  resetClientServices: (agentId) => apiRequest.post(
    formatApiPath(API_ENDPOINTS.AGENT.RESET_CLIENT_SERVICES, { agent_id: agentId })
  ),
  
  resetAgentClients: (agentId) => apiRequest.post(
    formatApiPath(API_ENDPOINTS.AGENT.RESET_AGENT_CLIENTS, { agent_id: agentId })
  ),
  
  /**
   * 统计信息
   */
  getStats: (agentId) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.AGENT.GET_STATS, { agent_id: agentId })
  ).then(res => extractResponseData(res.data)),
  
  getToolRecords: (agentId, limit = 50) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.AGENT.TOOL_RECORDS, { agent_id: agentId }),
    { params: { limit } }
  ).then(res => extractResponseData(res.data))
}