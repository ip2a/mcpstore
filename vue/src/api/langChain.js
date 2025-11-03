import { API_ENDPOINTS } from './config'
import { formatApiPath, extractResponseData } from './utils'
import { apiRequest } from './request'

/**
 * LangChain 集成 API 服务
 * 对应 MCPStore API v1.0.0 的 LangChain 集成端点
 */
export const langChainApi = {
  /**
   * Store 级别 LangChain 工具
   */
  getStoreTools: () => apiRequest.get(API_ENDPOINTS.LANGCHAIN.STORE_TOOLS)
    .then(res => extractResponseData(res.data, [])),
  
  getStoreServiceTools: (serviceName) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.LANGCHAIN.STORE_SERVICE_TOOLS, { service_name: serviceName })
  ).then(res => extractResponseData(res.data, [])),
  
  executeStoreTool: (toolName, args, kwargs) => apiRequest.post(API_ENDPOINTS.LANGCHAIN.STORE_TOOL_EXECUTE, {
    tool_name: toolName,
    args: args || [],
    kwargs: kwargs || {}
  }),
  
  getStoreToolInfo: (toolName) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.LANGCHAIN.STORE_TOOL_INFO, { tool_name: toolName })
  ).then(res => extractResponseData(res.data)),
  
  /**
   * Agent 级别 LangChain 工具
   */
  getAgentTools: (agentId) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.LANGCHAIN.AGENT_TOOLS, { agent_id: agentId })
  ).then(res => extractResponseData(res.data, [])),
  
  getAgentServiceTools: (agentId, serviceName) => apiRequest.get(
    formatApiPath(API_ENDPOINTS.LANGCHAIN.AGENT_SERVICE_TOOLS, { 
      agent_id: agentId,
      service_name: serviceName 
    })
  ).then(res => extractResponseData(res.data, [])),
  
  executeAgentTool: (agentId, toolName, args, kwargs) => apiRequest.post(
    formatApiPath(API_ENDPOINTS.LANGCHAIN.AGENT_TOOL_EXECUTE, { agent_id: agentId }),
    {
      tool_name: toolName,
      args: args || [],
      kwargs: kwargs || {}
    }
  )
}