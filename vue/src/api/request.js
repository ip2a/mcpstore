import axios from 'axios'
import { ElMessage } from 'element-plus'
import { API_BASE_URL, API_TIMEOUT_MS, API_VERSION } from './config'
import { handleApiError } from './utils'
import { STORAGE_KEYS } from '@/utils/constants'

// 创建axios实例
const request = axios.create({
  baseURL: API_BASE_URL,
  timeout: API_TIMEOUT_MS,
  headers: {
    'Content-Type': 'application/json',
    'X-API-Version': API_VERSION
  }
})

// 请求拦截器
request.interceptors.request.use(
  (config) => {
    // 添加时间戳防止缓存（仅GET请求）
    if (config.method === 'get') {
      config.params = {
        ...config.params,
        _t: Date.now()
      }
    }

    // 添加认证头（如果有token）
    const token = localStorage.getItem(STORAGE_KEYS.TOKEN)
    if (token) {
      config.headers.Authorization = `Bearer ${token}`
    }

    // 开发环境或启用日志时显示详细日志
    if (import.meta.env.DEV && import.meta.env.VITE_ENABLE_CONSOLE_LOG !== 'false') {
      console.log('🚀 [REQUEST]:', {
        method: config.method?.toUpperCase(),
        url: config.url,
        params: config.params,
        data: config.data
      })
    }

    return config
  },
  (error) => {
    console.error('❌ [REQUEST ERROR]:', error)
    return Promise.reject(handleApiError(error, 'Request'))
  }
)

// 响应拦截器
request.interceptors.response.use(
  (response) => {
    const { data } = response

    // 开发环境或启用日志时显示详细日志
    if (import.meta.env.DEV && import.meta.env.VITE_ENABLE_CONSOLE_LOG !== 'false') {
      console.log('✅ [RESPONSE]:', {
        status: response.status,
        url: response.config.url,
        data: data
      })
    }
    
    // 统一的响应格式验证
    if (data && typeof data === 'object') {
      // 检查API响应格式
      if ('success' in data && !data.success) {
        // 业务错误，返回错误对象
        const error = new Error(data.message || 'API request failed')
        error.code = data.error?.code
        error.details = data.error?.details
        error.response = response
        return Promise.reject(error)
      }

      // 成功响应，返回完整数据
      return response
    }

    // 非对象响应，直接返回
    return response
  },
  (error) => {
    const apiError = handleApiError(error, 'Response')
    
    // 根据错误类型显示用户友好的消息
    let userMessage = apiError.message
    
    switch (apiError.type) {
      case 'NETWORK_ERROR':
        userMessage = '网络连接失败，请检查网络设置'
        break
      case 'TIMEOUT_ERROR':
        userMessage = '请求超时，请稍后重试'
        break
      case 'UNAUTHORIZED':
        userMessage = '未授权访问，请重新登录'
        // 清除无效的token
        localStorage.removeItem(STORAGE_KEYS.TOKEN)
        break
      case 'FORBIDDEN':
        userMessage = '权限不足，无法访问该资源'
        break
      case 'NOT_FOUND':
        userMessage = '请求的资源不存在'
        break
      case 'SERVICE_UNAVAILABLE':
        userMessage = '服务暂时不可用，请稍后重试'
        break
      default:
        userMessage = apiError.message || '操作失败，请稍后重试'
    }
    
    // 显示错误消息（除了静默错误）
    if (!error.config?.silent) {
      ElMessage.error(userMessage)
    }
    
    return Promise.reject(apiError)
  }
)

// 通用请求方法
export const apiRequest = {
  get: (url, config = {}) => request.get(url, config),
  post: (url, data = {}, config = {}) => request.post(url, data, config),
  put: (url, data = {}, config = {}) => request.put(url, data, config),
  delete: (url, config = {}) => request.delete(url, config),
  patch: (url, data = {}, config = {}) => request.patch(url, data, config)
}

// 文件上传请求
export const uploadRequest = (url, formData, onProgress, config = {}) => {
  return request.post(url, formData, {
    ...config,
    headers: {
      'Content-Type': 'multipart/form-data',
      ...config.headers
    },
    onUploadProgress: (progressEvent) => {
      if (onProgress && progressEvent.total) {
        const progress = Math.round((progressEvent.loaded * 100) / progressEvent.total)
        onProgress(progress)
      }
    }
  })
}

// 下载文件请求
export const downloadRequest = async (url, params = {}, filename = null) => {
  try {
    const response = await request.get(url, {
      params,
      responseType: 'blob'
    })
    
    // 从响应头获取文件名
    const contentDisposition = response.headers['content-disposition']
    let defaultFilename = filename || 'download'
    
    if (contentDisposition) {
      const filenameMatch = contentDisposition.match(/filename[^;=\n]*=((['"]).*?\2|[^;\n]*)/)
      if (filenameMatch && filenameMatch[1]) {
        defaultFilename = filenameMatch[1].replace(/['"]/g, '')
      }
    }
    
    const blob = new Blob([response.data])
    const downloadUrl = window.URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = downloadUrl
    link.download = defaultFilename
    document.body.appendChild(link)
    link.click()
    document.body.removeChild(link)
    window.URL.revokeObjectURL(downloadUrl)
    
    return { success: true, filename: defaultFilename }
  } catch (error) {
    console.error('Download failed:', error)
    throw error
  }
}

// 批量请求（支持并发控制）
export const batchRequest = async (requests, concurrency = 5) => {
  const results = []
  
  for (let i = 0; i < requests.length; i += concurrency) {
    const batch = requests.slice(i, i + concurrency)
    const batchResults = await Promise.allSettled(
      batch.map(req => {
        const { method, url, data, params, config = {} } = req
        return request[method](url, method === 'get' ? { ...config, params } : { ...config, data })
      })
    )
    results.push(...batchResults)
  }
  
  return results
}

// 重试请求（支持指数退避）
export const retryRequest = async (requestFn, maxRetries = 3, baseDelay = 1000) => {
  let lastError
  
  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      return await requestFn()
    } catch (error) {
      lastError = error
      
      if (attempt === maxRetries) {
        break
      }
      
      // 指数退避
      const delay = baseDelay * Math.pow(2, attempt - 1)
      await new Promise(resolve => setTimeout(resolve, delay))
    }
  }
  
  throw lastError
}

// 取消请求控制器
export const createCancelToken = () => {
  const source = axios.CancelToken.source()
  return {
    token: source.token,
    cancel: source.cancel
  }
}

// WebSocket 连接管理
export const createWebSocket = (url, options = {}) => {
  const ws = new WebSocket(url)
  
  ws.onopen = () => {
    if (import.meta.env.DEV) console.log('WebSocket connected')
    options.onOpen?.()
  }
  
  ws.onmessage = (event) => {
    try {
      const data = JSON.parse(event.data)
      options.onMessage?.(data)
    } catch (error) {
      if (import.meta.env.DEV) console.error('WebSocket message parse error:', error)
      options.onError?.(error)
    }
  }
  
  ws.onclose = () => {
    if (import.meta.env.DEV) console.log('WebSocket disconnected')
    options.onClose?.()
    
    // 自动重连
    if (options.reconnect !== false) {
      setTimeout(() => {
        createWebSocket(url, options)
      }, options.reconnectDelay || 3000)
    }
  }
  
  ws.onerror = (error) => {
    if (import.meta.env.DEV) console.error('WebSocket error:', error)
    options.onError?.(error)
  }
  
  return ws
}

// 请求缓存
const requestCache = new Map()
export const cachedRequest = async (key, requestFn, ttl = 60000) => {
  const cached = requestCache.get(key)
  
  if (cached && Date.now() - cached.timestamp < ttl) {
    return cached.data
  }
  
  const data = await requestFn()
  requestCache.set(key, {
    data,
    timestamp: Date.now()
  })
  
  return data
}

// 清除缓存
export const clearRequestCache = (pattern = null) => {
  if (pattern) {
    const regex = new RegExp(pattern)
    for (const key of requestCache.keys()) {
      if (regex.test(key)) {
        requestCache.delete(key)
      }
    }
  } else {
    requestCache.clear()
  }
}

export default request
