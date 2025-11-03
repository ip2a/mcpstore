import { API_STATUS_CODES, ERROR_TYPES } from './config'

/**
 * 统一的 API 错误处理类
 */
export class APIError extends Error {
  constructor(message, type = ERROR_TYPES.INTERNAL_ERROR, statusCode = API_STATUS_CODES.INTERNAL_ERROR, details = null) {
    super(message)
    this.name = 'APIError'
    this.type = type
    this.statusCode = statusCode
    this.details = details
    this.timestamp = new Date().toISOString()
  }
}

/**
 * 处理 API 响应错误
 */
export function handleApiError(error, context = '') {
  console.error(`[API Error] ${context}:`, error)

  if (error.response) {
    // 服务器响应了错误状态码
    const { status, data } = error.response
    const message = data?.error?.message || data?.message || error.message
    const type = data?.error?.code || ERROR_TYPES.INTERNAL_ERROR
    
    return new APIError(
      `${context}: ${message}`,
      type,
      status,
      data?.error?.details || data
    )
  } else if (error.request) {
    // 请求已发出但没有收到响应
    return new APIError(
      `${context}: Network error - no response received`,
      ERROR_TYPES.INTERNAL_ERROR,
      0,
      { request: error.request }
    )
  } else {
    // 请求设置时出错
    return new APIError(
      `${context}: ${error.message}`,
      ERROR_TYPES.INTERNAL_ERROR,
      0,
      { originalError: error }
    )
  }
}

/**
 * 验证 API 响应格式
 */
export function validateApiResponse(response) {
  if (!response || typeof response !== 'object') {
    throw new APIError('Invalid API response format', ERROR_TYPES.VALIDATION_ERROR)
  }

  // 检查是否有 success 字段
  if ('success' in response && typeof response.success !== 'boolean') {
    throw new APIError('Invalid success field in response', ERROR_TYPES.VALIDATION_ERROR)
  }

  return response
}

/**
 * 提取响应数据
 */
export function extractResponseData(response, defaultValue = null) {
  const validated = validateApiResponse(response)
  
  if (validated.success) {
    return validated.data ?? defaultValue
  } else {
    throw new APIError(
      validated.message || 'API request failed',
      validated.error?.code || ERROR_TYPES.INTERNAL_ERROR,
      validated.error?.statusCode || API_STATUS_CODES.INTERNAL_ERROR,
      validated.error
    )
  }
}

/**
 * 格式化 API 路径参数
 */
export function formatApiPath(path, params = {}) {
  let formattedPath = path
  
  for (const [key, value] of Object.entries(params)) {
    formattedPath = formattedPath.replace(`{${key}}`, encodeURIComponent(value))
  }
  
  return formattedPath
}

/**
 * 创建查询字符串
 */
export function buildQueryString(params = {}) {
  const searchParams = new URLSearchParams()
  
  for (const [key, value] of Object.entries(params)) {
    if (value !== null && value !== undefined) {
      if (Array.isArray(value)) {
        value.forEach(item => searchParams.append(key, item))
      } else {
        searchParams.append(key, value)
      }
    }
  }
  
  return searchParams.toString()
}


/**
 * 深度合并对象
 */
export function deepMerge(target, source) {
  const output = Object.assign({}, target)
  
  if (isObject(target) && isObject(source)) {
    Object.keys(source).forEach(key => {
      if (isObject(source[key])) {
        if (!(key in target))
          Object.assign(output, { [key]: source[key] })
        else
          output[key] = deepMerge(target[key], source[key])
      } else {
        Object.assign(output, { [key]: source[key] })
      }
    })
  }
  
  return output
}

function isObject(item) {
  return item && typeof item === 'object' && !Array.isArray(item)
}

/**
 * 生成唯一 ID
 */
export function generateUniqueId() {
  return Date.now().toString(36) + Math.random().toString(36).substr(2)
}

/**
 * 格式化文件大小
 */
export function formatFileSize(bytes) {
  if (bytes === 0) return '0 Bytes'
  
  const k = 1024
  const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

/**
 * 格式化持续时间
 */
export function formatDuration(ms) {
  if (ms < 1000) return `${ms}ms`
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`
  if (ms < 3600000) return `${(ms / 60000).toFixed(1)}min`
  
  const hours = Math.floor(ms / 3600000)
  const minutes = Math.floor((ms % 3600000) / 60000)
  
  return `${hours}h ${minutes}m`
}
