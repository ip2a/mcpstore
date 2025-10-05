// MCP 专用 HTTP 客户端：兼容 MCPStore API 的 { success, data, message } 返回格式
import axios, { AxiosRequestConfig, AxiosResponse } from 'axios'

const { VITE_API_URL, VITE_WITH_CREDENTIALS } = import.meta.env

// 开发环境走 Vite 代理 '/api'，生产环境用显式 API 地址
const BASE_URL = import.meta.env.DEV ? '/api' : VITE_API_URL

const instance = axios.create({
  baseURL: BASE_URL,
  withCredentials: VITE_WITH_CREDENTIALS === 'true',
  timeout: 15000
})

// 简单请求拦截：JSON 序列化
instance.interceptors.request.use((config) => {
  if (config.data && !(config.data instanceof FormData) && !config.headers?.['Content-Type']) {
    config.headers = config.headers || {}
    config.headers['Content-Type'] = 'application/json'
    config.data = JSON.stringify(config.data)
  }
  return config
})

// 简单响应拦截：直接返回后端 JSON，不根据 code/msg 判断
instance.interceptors.response.use(
  (response: AxiosResponse) => response.data,
  (error) => Promise.reject(error)
)

export default {
  get<T = any>(config: AxiosRequestConfig) {
    return instance.request<T>({ ...config, method: 'GET' })
  },
  post<T = any>(config: AxiosRequestConfig) {
    return instance.request<T>({ ...config, method: 'POST' })
  },
  put<T = any>(config: AxiosRequestConfig) {
    return instance.request<T>({ ...config, method: 'PUT' })
  },
  del<T = any>(config: AxiosRequestConfig) {
    return instance.request<T>({ ...config, method: 'DELETE' })
  }
}

