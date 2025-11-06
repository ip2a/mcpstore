/**
 * 统一的 API 调用 Composable
 * 封装常见的 API 调用模式，自动处理加载状态、错误处理和数据管理
 */

import { ref, computed, unref } from 'vue'
import { useErrorHandler } from './useErrorHandler'
import { useLoadingState } from './useLoadingState'

/**
 * 创建 API 调用管理器
 * @param {Function} apiFn - API 调用函数
 * @param {Object} options - 配置选项
 * @returns {Object} API 调用管理器实例
 */
export function useApi(apiFn, options = {}) {
  const {
    immediate = false,
    initialData = null,
    loadingKey = 'api',
    errorHandler = null,
    transform = null,
    onSuccess = null,
    onError = null,
    cache = false,
    cacheTime = 60000
  } = options

  // 使用 composables
  const errorHandlerInstance = errorHandler || useErrorHandler({ source: 'api' })
  const loadingStateInstance = useLoadingState({ [loadingKey]: false })

  // 状态
  const data = ref(initialData)
  const error = ref(null)
  const isLoading = computed(() => loadingStateInstance.getLoading(loadingKey))
  const isReady = ref(false)
  const lastFetchTime = ref(null)

  // 缓存
  const cacheData = cache ? new Map() : null
  const cacheTimestamps = cache ? new Map() : null

  /**
   * 执行 API 调用
   * @param  {...any} args - API 函数参数
   * @returns {Promise} API 调用结果
   */
  const execute = async (...args) => {
    // 检查缓存
    if (cache) {
      const cacheKey = JSON.stringify(args)
      const cachedValue = cacheData.get(cacheKey)
      const cacheTimestamp = cacheTimestamps.get(cacheKey)
      
      if (cachedValue && cacheTimestamp && (Date.now() - cacheTimestamp < cacheTime)) {
        data.value = cachedValue
        return cachedValue
      }
    }

    loadingStateInstance.setLoading(loadingKey, true)
    error.value = null

    try {
      const response = await apiFn(...args)
      
      // 提取数据
      let result = response?.data ?? response
      
      // 转换数据
      if (transform) {
        result = transform(result)
      }

      data.value = result
      isReady.value = true
      lastFetchTime.value = Date.now()

      // 缓存结果
      if (cache) {
        const cacheKey = JSON.stringify(args)
        cacheData.set(cacheKey, result)
        cacheTimestamps.set(cacheKey, Date.now())
      }

      // 成功回调
      if (onSuccess) {
        onSuccess(result, response)
      }

      return result
    } catch (err) {
      error.value = err
      
      // 错误处理
      const errorObj = errorHandlerInstance.handleApiError(err, 'API调用失败')
      
      // 错误回调
      if (onError) {
        onError(errorObj, err)
      }

      throw err
    } finally {
      loadingStateInstance.setLoading(loadingKey, false)
    }
  }

  /**
   * 重新执行 API 调用（刷新）
   * @param  {...any} args - API 函数参数
   */
  const refresh = async (...args) => {
    // 清除缓存
    if (cache) {
      clearCache()
    }
    return execute(...args)
  }

  /**
   * 重置状态
   */
  const reset = () => {
    data.value = initialData
    error.value = null
    isReady.value = false
    lastFetchTime.value = null
    loadingStateInstance.reset(loadingKey)
    
    if (cache) {
      clearCache()
    }
  }

  /**
   * 清除缓存
   */
  const clearCache = () => {
    if (cache) {
      cacheData.clear()
      cacheTimestamps.clear()
    }
  }

  // 立即执行
  if (immediate) {
    execute()
  }

  return {
    // 状态
    data,
    error,
    isLoading,
    isReady,
    lastFetchTime,
    
    // 方法
    execute,
    refresh,
    reset,
    clearCache
  }
}

/**
 * 批量 API 调用管理器
 * @param {Array<Function>} apiFns - API 调用函数数组
 * @param {Object} options - 配置选项
 * @returns {Object} 批量 API 调用管理器实例
 */
export function useBatchApi(apiFns, options = {}) {
  const {
    immediate = false,
    parallel = true,
    errorHandler = null,
    onComplete = null
  } = options

  const errorHandlerInstance = errorHandler || useErrorHandler({ source: 'batch-api' })
  const loadingStateInstance = useLoadingState({ batch: false })

  const results = ref([])
  const errors = ref([])
  const isLoading = computed(() => loadingStateInstance.getLoading('batch'))
  const isComplete = ref(false)

  /**
   * 执行所有 API 调用
   * @param  {...any} args - 传递给所有 API 函数的参数
   */
  const executeAll = async (...args) => {
    loadingStateInstance.setLoading('batch', true)
    errors.value = []
    isComplete.value = false

    try {
      if (parallel) {
        // 并行执行
        const promises = apiFns.map(fn => 
          fn(...args).catch(err => {
            errors.value.push(err)
            errorHandlerInstance.handleApiError(err, '批量API调用部分失败')
            return null
          })
        )
        results.value = await Promise.all(promises)
      } else {
        // 串行执行
        results.value = []
        for (const fn of apiFns) {
          try {
            const result = await fn(...args)
            results.value.push(result)
          } catch (err) {
            errors.value.push(err)
            errorHandlerInstance.handleApiError(err, '批量API调用部分失败')
            results.value.push(null)
          }
        }
      }

      isComplete.value = true

      if (onComplete) {
        onComplete(results.value, errors.value)
      }

      return results.value
    } finally {
      loadingStateInstance.setLoading('batch', false)
    }
  }

  /**
   * 重置状态
   */
  const reset = () => {
    results.value = []
    errors.value = []
    isComplete.value = false
    loadingStateInstance.reset('batch')
  }

  if (immediate) {
    executeAll()
  }

  return {
    // 状态
    results,
    errors,
    isLoading,
    isComplete,
    
    // 方法
    executeAll,
    reset
  }
}

/**
 * 轮询 API 调用管理器
 * @param {Function} apiFn - API 调用函数
 * @param {Object} options - 配置选项
 * @returns {Object} 轮询管理器实例
 */
export function usePollingApi(apiFn, options = {}) {
  const {
    interval = 5000,
    immediate = false,
    enabled = true,
    errorHandler = null,
    onSuccess = null,
    onError = null
  } = options

  const errorHandlerInstance = errorHandler || useErrorHandler({ source: 'polling-api' })

  const data = ref(null)
  const error = ref(null)
  const isPolling = ref(false)
  const pollCount = ref(0)
  let timerId = null

  /**
   * 执行单次轮询
   */
  const poll = async () => {
    try {
      const response = await apiFn()
      const result = response?.data ?? response
      
      data.value = result
      error.value = null
      pollCount.value++

      if (onSuccess) {
        onSuccess(result, response)
      }

      return result
    } catch (err) {
      error.value = err
      errorHandlerInstance.handleApiError(err, '轮询API调用失败', { silent: true })
      
      if (onError) {
        onError(err)
      }

      throw err
    }
  }

  /**
   * 开始轮询
   */
  const start = () => {
    if (isPolling.value) return

    isPolling.value = true
    
    // 立即执行一次
    if (immediate) {
      poll()
    }

    // 设置定时器
    timerId = setInterval(() => {
      if (enabled) {
        poll()
      }
    }, unref(interval))
  }

  /**
   * 停止轮询
   */
  const stop = () => {
    if (timerId) {
      clearInterval(timerId)
      timerId = null
    }
    isPolling.value = false
  }

  /**
   * 重置状态
   */
  const reset = () => {
    stop()
    data.value = null
    error.value = null
    pollCount.value = 0
  }

  // 自动开始
  if (enabled && immediate) {
    start()
  }

  return {
    // 状态
    data,
    error,
    isPolling,
    pollCount,
    
    // 方法
    start,
    stop,
    reset,
    poll
  }
}

/**
 * 分页 API 调用管理器
 * @param {Function} apiFn - API 调用函数
 * @param {Object} options - 配置选项
 * @returns {Object} 分页管理器实例
 */
export function usePaginationApi(apiFn, options = {}) {
  const {
    initialPage = 1,
    initialPageSize = 20,
    immediate = false,
    errorHandler = null,
    transform = null
  } = options

  const errorHandlerInstance = errorHandler || useErrorHandler({ source: 'pagination-api' })
  const loadingStateInstance = useLoadingState({ pagination: false })

  const data = ref([])
  const total = ref(0)
  const currentPage = ref(initialPage)
  const pageSize = ref(initialPageSize)
  const isLoading = computed(() => loadingStateInstance.getLoading('pagination'))

  /**
   * 加载数据
   * @param {number} page - 页码
   * @param {number} size - 每页数量
   */
  const load = async (page = currentPage.value, size = pageSize.value) => {
    loadingStateInstance.setLoading('pagination', true)

    try {
      const params = { page, page_size: size, pageSize: size }
      const response = await apiFn(params)
      let result = response?.data ?? response

      if (transform) {
        result = transform(result)
      }

      data.value = result.items || result.data || result
      total.value = result.total || result.count || data.value.length
      currentPage.value = page
      pageSize.value = size

      return result
    } catch (err) {
      errorHandlerInstance.handleApiError(err, '加载分页数据失败')
      throw err
    } finally {
      loadingStateInstance.setLoading('pagination', false)
    }
  }

  /**
   * 下一页
   */
  const nextPage = () => {
    const totalPages = Math.ceil(total.value / pageSize.value)
    if (currentPage.value < totalPages) {
      return load(currentPage.value + 1)
    }
  }

  /**
   * 上一页
   */
  const prevPage = () => {
    if (currentPage.value > 1) {
      return load(currentPage.value - 1)
    }
  }

  /**
   * 跳转到指定页
   * @param {number} page - 页码
   */
  const goToPage = (page) => {
    return load(page)
  }

  /**
   * 刷新当前页
   */
  const refresh = () => {
    return load()
  }

  /**
   * 重置
   */
  const reset = () => {
    data.value = []
    total.value = 0
    currentPage.value = initialPage
    pageSize.value = initialPageSize
  }

  if (immediate) {
    load()
  }

  return {
    // 状态
    data,
    total,
    currentPage,
    pageSize,
    isLoading,
    
    // 计算属性
    totalPages: computed(() => Math.ceil(total.value / pageSize.value)),
    hasNextPage: computed(() => currentPage.value < Math.ceil(total.value / pageSize.value)),
    hasPrevPage: computed(() => currentPage.value > 1),
    
    // 方法
    load,
    nextPage,
    prevPage,
    goToPage,
    refresh,
    reset
  }
}

