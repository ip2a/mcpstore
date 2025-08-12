import { apiRequest } from './request'

/**
 * 工具管理相关API
 */

// Store级别工具管理
export const storeToolsAPI = {
  // 获取工具列表
  getToolsList: () => apiRequest.get('/for_store/list_tools'),
  
  // 执行工具
  executeTool: (toolName, params) => apiRequest.post('/for_store/use_tool', {
    tool_name: toolName,
    args: params
  }),
  
  // 获取工具详情
  getToolDetails: (toolName) => apiRequest.post('/for_store/get_tool_info', {
    tool_name: toolName
  }),
  
  // 获取工具执行记录（合并历史和统计）
  getToolRecords: (limit = 50) => apiRequest.get('/for_store/tool_records', {
    params: { limit }
  })
}

// Agent级别工具管理
export const agentToolsAPI = {
  // 获取Agent工具列表
  getToolsList: (agentId) => apiRequest.get(`/for_agent/${agentId}/list_tools`),
  
  // Agent执行工具
  executeTool: (agentId, toolName, params) => apiRequest.post(`/for_agent/${agentId}/use_tool`, {
    tool_name: toolName,
    args: params
  }),
  
  // 获取Agent工具详情
  getToolDetails: (agentId, toolName) => apiRequest.post(`/for_agent/${agentId}/get_tool_info`, {
    tool_name: toolName
  }),
  
  // 获取Agent工具执行记录（合并历史和统计）
  getToolRecords: (agentId, limit = 50) => apiRequest.get(`/for_agent/${agentId}/tool_records`, {
    params: { limit }
  })
}

// 工具验证函数
export const validateToolParams = (tool, params) => {
  const errors = []
  
  if (!tool.inputSchema || !tool.inputSchema.properties) {
    return { isValid: true, errors: [] }
  }
  
  const required = tool.inputSchema.required || []
  const properties = tool.inputSchema.properties
  
  // 检查必需参数
  for (const requiredParam of required) {
    if (!params.hasOwnProperty(requiredParam) || params[requiredParam] === null || params[requiredParam] === undefined) {
      errors.push(`缺少必需参数: ${requiredParam}`)
    }
  }
  
  // 检查参数类型
  for (const [paramName, paramValue] of Object.entries(params)) {
    if (properties[paramName]) {
      const expectedType = properties[paramName].type
      const actualType = typeof paramValue
      
      if (expectedType === 'string' && actualType !== 'string') {
        errors.push(`参数 ${paramName} 应为字符串类型`)
      } else if (expectedType === 'number' && actualType !== 'number') {
        errors.push(`参数 ${paramName} 应为数字类型`)
      } else if (expectedType === 'boolean' && actualType !== 'boolean') {
        errors.push(`参数 ${paramName} 应为布尔类型`)
      }
    }
  }
  
  return {
    isValid: errors.length === 0,
    errors
  }
}

// 工具参数模板生成
export const generateToolParamsTemplate = (tool) => {
  if (!tool.inputSchema || !tool.inputSchema.properties) {
    return {}
  }
  
  const template = {}
  const properties = tool.inputSchema.properties
  
  for (const [paramName, paramSchema] of Object.entries(properties)) {
    switch (paramSchema.type) {
      case 'string':
        template[paramName] = paramSchema.default || ''
        break
      case 'number':
        template[paramName] = paramSchema.default || 0
        break
      case 'boolean':
        template[paramName] = paramSchema.default || false
        break
      case 'array':
        template[paramName] = paramSchema.default || []
        break
      case 'object':
        template[paramName] = paramSchema.default || {}
        break
      default:
        template[paramName] = paramSchema.default || null
    }
  }
  
  return template
}
