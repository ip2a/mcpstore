/**
 * 统一的错误处理 Composable
 * 提供错误状态管理、错误记录和错误通知功能
 */

import { ref, computed } from 'vue'
import { ElMessage, ElNotification } from 'element-plus'

/**
 * 创建错误处理器
 * @param {Object} options - 配置选项
 * @param {number} options.maxErrors - 最大错误记录数，默认 100
 * @param {boolean} options.showNotification - 是否显示通知，默认 true
 * @param {string} options.source - 错误来源标识
 * @returns {Object} 错误处理器实例
 */
export function useErrorHandler(options = {}) {
  const {
    maxErrors = 100,
    showNotification = true,
    source = 'unknown'
  } = options

  // 状态
  const errors = ref([])
  const lastError = ref(null)

  // 计算属性
  /**
   * 是否有错误
   */
  const hasErrors = computed(() => errors.value.length > 0)

  /**
   * 最近的错误列表（最多5个）
   */
  const recentErrors = computed(() => {
    return errors.value.slice(-5).reverse()
  })

  /**
   * 错误数量
   */
  const errorCount = computed(() => errors.value.length)

  /**
   * 添加错误
   * @param {Error|string|Object} error - 错误对象或错误信息
   * @param {Object} additionalInfo - 额外信息
   * @returns {Object} 错误对象
   */
  const addError = (error, additionalInfo = {}) => {
    const errorObj = {
      id: Date.now() + Math.random(), // 确保唯一性
      message: error?.message || error || '未知错误',
      stack: error?.stack,
      timestamp: new Date().toISOString(),
      type: error?.type || additionalInfo.type || 'error',
      source: additionalInfo.source || source,
      code: error?.code || additionalInfo.code,
      details: error?.details || additionalInfo.details
    }

    errors.value.push(errorObj)
    lastError.value = errorObj

    // 限制错误数量
    if (errors.value.length > maxErrors) {
      errors.value = errors.value.slice(-maxErrors)
    }

    // 显示通知
    if (showNotification && !additionalInfo.silent) {
      showErrorNotification(errorObj)
    }

    return errorObj
  }

  /**
   * 显示错误通知
   * @param {Object} errorObj - 错误对象
   */
  const showErrorNotification = (errorObj) => {
    const message = errorObj.message || '操作失败'
    
    // 根据错误类型选择通知方式
    if (errorObj.type === 'warning') {
      ElMessage.warning({
        message,
        duration: 3000,
        showClose: true
      })
    } else if (errorObj.type === 'critical' || errorObj.type === 'fatal') {
      ElNotification.error({
        title: '严重错误',
        message,
        duration: 0, // 不自动关闭
        position: 'top-right'
      })
    } else {
      ElMessage.error({
        message,
        duration: 3000,
        showClose: true
      })
    }
  }

  /**
   * 清除所有错误
   */
  const clearErrors = () => {
    errors.value = []
    lastError.value = null
  }

  /**
   * 移除特定错误
   * @param {number|string} errorId - 错误ID
   */
  const removeError = (errorId) => {
    const index = errors.value.findIndex(error => error.id === errorId)
    if (index > -1) {
      errors.value.splice(index, 1)
      
      // 如果删除的是最后一个错误，更新 lastError
      if (lastError.value?.id === errorId) {
        lastError.value = errors.value[errors.value.length - 1] || null
      }
    }
  }

  /**
   * 根据类型清除错误
   * @param {string} type - 错误类型
   */
  const clearErrorsByType = (type) => {
    errors.value = errors.value.filter(error => error.type !== type)
    
    // 如果 lastError 被清除，更新为最新的错误
    if (lastError.value?.type === type) {
      lastError.value = errors.value[errors.value.length - 1] || null
    }
  }

  /**
   * 根据来源清除错误
   * @param {string} errorSource - 错误来源
   */
  const clearErrorsBySource = (errorSource) => {
    errors.value = errors.value.filter(error => error.source !== errorSource)
    
    if (lastError.value?.source === errorSource) {
      lastError.value = errors.value[errors.value.length - 1] || null
    }
  }

  /**
   * 获取特定类型的错误
   * @param {string} type - 错误类型
   * @returns {Array} 错误列表
   */
  const getErrorsByType = (type) => {
    return errors.value.filter(error => error.type === type)
  }

  /**
   * 获取特定来源的错误
   * @param {string} errorSource - 错误来源
   * @returns {Array} 错误列表
   */
  const getErrorsBySource = (errorSource) => {
    return errors.value.filter(error => error.source === errorSource)
  }

  /**
   * 处理API错误的辅助函数
   * @param {Error} error - API错误对象
   * @param {string} context - 错误上下文
   * @param {Object} options - 额外选项
   */
  const handleApiError = (error, context = '', options = {}) => {
    let errorMessage = '操作失败'
    let errorType = 'error'

    // 解析不同类型的错误
    if (error.response) {
      // HTTP 错误响应
      const status = error.response.status
      const data = error.response.data

      switch (status) {
        case 400:
          errorMessage = data?.message || '请求参数错误'
          errorType = 'validation'
          break
        case 401:
          errorMessage = '未授权，请重新登录'
          errorType = 'auth'
          break
        case 403:
          errorMessage = '权限不足'
          errorType = 'permission'
          break
        case 404:
          errorMessage = data?.message || '资源不存在'
          errorType = 'not-found'
          break
        case 500:
          errorMessage = '服务器内部错误'
          errorType = 'server'
          break
        case 503:
          errorMessage = '服务暂时不可用'
          errorType = 'unavailable'
          break
        default:
          errorMessage = data?.message || error.message || `请求失败 (${status})`
      }
    } else if (error.request) {
      // 网络错误
      errorMessage = '网络连接失败，请检查网络设置'
      errorType = 'network'
    } else {
      // 其他错误
      errorMessage = error.message || '未知错误'
    }

    // 添加上下文信息
    if (context) {
      errorMessage = `${context}: ${errorMessage}`
    }

    return addError(error, {
      type: errorType,
      message: errorMessage,
      ...options
    })
  }

  /**
   * 包装异步函数，自动处理错误
   * @param {Function} asyncFn - 异步函数
   * @param {Object} options - 配置选项
   * @returns {Function} 包装后的函数
   */
  const withErrorHandling = (asyncFn, options = {}) => {
    return async (...args) => {
      try {
        return await asyncFn(...args)
      } catch (error) {
        handleApiError(error, options.context, {
          source: options.source || source,
          silent: options.silent
        })
        
        if (options.rethrow) {
          throw error
        }
        
        return options.fallbackValue
      }
    }
  }

  return {
    // 状态
    errors,
    lastError,
    
    // 计算属性
    hasErrors,
    recentErrors,
    errorCount,
    
    // 方法
    addError,
    clearErrors,
    removeError,
    clearErrorsByType,
    clearErrorsBySource,
    getErrorsByType,
    getErrorsBySource,
    handleApiError,
    withErrorHandling,
    showErrorNotification
  }
}

/**
 * 创建全局错误处理器（单例模式）
 */
let globalErrorHandler = null

export function useGlobalErrorHandler() {
  if (!globalErrorHandler) {
    globalErrorHandler = useErrorHandler({
      maxErrors: 200,
      source: 'global'
    })
  }
  return globalErrorHandler
}

/**
 * 重置全局错误处理器
 */
export function resetGlobalErrorHandler() {
  if (globalErrorHandler) {
    globalErrorHandler.clearErrors()
    globalErrorHandler = null
  }
}

