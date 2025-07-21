import axios from 'axios'
import { ElMessage, ElMessageBox } from 'element-plus'
import NProgress from 'nprogress'

// 创建axios实例
const request = axios.create({
  baseURL: import.meta.env.VITE_API_BASE_URL || 'http://localhost:18200',
  timeout: parseInt(import.meta.env.VITE_API_TIMEOUT) || 5000,
  headers: {
    'Content-Type': 'application/json'
  }
})

// 请求拦截器
request.interceptors.request.use(
  (config) => {
    // 添加时间戳防止缓存
    if (config.method === 'get') {
      config.params = {
        ...config.params,
        _t: Date.now()
      }
    }

    // 打印请求信息（开发环境）
    if (import.meta.env.DEV) {
      console.log(`🚀 API Request: ${config.method?.toUpperCase()} ${config.url}`, {
        params: config.params,
        data: config.data
      })
    }

    return config
  },
  (error) => {
    console.error('Request Error:', error)
    return Promise.reject(error)
  }
)

// 响应拦截器
request.interceptors.response.use(
  (response) => {
    const { data } = response
    
    // 打印响应信息（开发环境）
    if (import.meta.env.DEV) {
      console.log(`✅ API Response: ${response.config.method?.toUpperCase()} ${response.config.url}`, data)
    }
    
    // 检查业务状态码
    if (data && typeof data === 'object') {
      if (data.success === false) {
        // 业务错误 - 不在拦截器中显示错误消息，让组件自己处理
        console.warn('API业务错误:', data.message || '请求失败')
        // 仍然返回数据，让组件自己判断success字段
        return { data }
      }

      // 检查是否有错误字段
      if (data.error && typeof data.error === 'string') {
        console.warn('API错误字段:', data.error)
        return Promise.reject(new Error(data.error))
      }

      // 返回完整的响应数据，包装在response对象中
      return { data }
    }

    // 直接返回响应数据，包装在response对象中
    return { data }
  },
  (error) => {
    console.error('Response Error:', error)
    
    let errorMessage = '网络错误'
    
    if (error.response) {
      // 服务器响应错误
      const { status, data } = error.response
      
      switch (status) {
        case 400:
          errorMessage = data?.message || '请求参数错误'
          break
        case 401:
          errorMessage = '未授权访问'
          break
        case 403:
          errorMessage = '禁止访问'
          break
        case 404:
          errorMessage = '请求的资源不存在'
          break
        case 500:
          errorMessage = data?.message || '服务器内部错误'
          break
        case 502:
          errorMessage = '网关错误'
          break
        case 503:
          errorMessage = '服务不可用'
          break
        default:
          errorMessage = data?.message || `请求失败 (${status})`
      }
    } else if (error.request) {
      // 网络错误
      if (error.code === 'ECONNABORTED') {
        errorMessage = '请求超时'
      } else if (error.message.includes('Network Error')) {
        errorMessage = '网络连接失败，请检查后端服务是否启动'
      } else {
        errorMessage = '网络错误'
      }
    } else {
      errorMessage = error.message || '未知错误'
    }
    
    // 显示错误消息
    ElMessage.error(errorMessage)
    
    return Promise.reject(error)
  }
)

// 通用请求方法
export const apiRequest = {
  get: (url, params = {}) => request.get(url, { params }),
  post: (url, data = {}) => request.post(url, data),
  put: (url, data = {}) => request.put(url, data),
  delete: (url, params = {}) => request.delete(url, { params }),
  patch: (url, data = {}) => request.patch(url, data)
}

// 文件上传请求
export const uploadRequest = (url, formData, onProgress) => {
  return request.post(url, formData, {
    headers: {
      'Content-Type': 'multipart/form-data'
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
export const downloadRequest = (url, params = {}, filename) => {
  return request.get(url, {
    params,
    responseType: 'blob'
  }).then(response => {
    const blob = new Blob([response.data])
    const downloadUrl = window.URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = downloadUrl
    link.download = filename || 'download'
    document.body.appendChild(link)
    link.click()
    document.body.removeChild(link)
    window.URL.revokeObjectURL(downloadUrl)
  })
}

// 批量请求
export const batchRequest = (requests) => {
  return Promise.allSettled(requests.map(req => {
    const { method, url, data, params } = req
    return request[method](url, method === 'get' ? { params } : data)
  }))
}

// 重试请求
export const retryRequest = (requestFn, maxRetries = 3, delay = 1000) => {
  return new Promise((resolve, reject) => {
    let retries = 0
    
    const attempt = () => {
      requestFn()
        .then(resolve)
        .catch(error => {
          retries++
          if (retries < maxRetries) {
            setTimeout(attempt, delay * retries)
          } else {
            reject(error)
          }
        })
    }
    
    attempt()
  })
}

export default request
