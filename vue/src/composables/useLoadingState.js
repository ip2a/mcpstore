/**
 * 统一的加载状态管理 Composable
 * 提供细粒度的加载状态管理，支持多个独立的加载状态
 */

import { ref, computed, reactive } from 'vue'

/**
 * 创建加载状态管理器
 * @param {Object|Array} initialStates - 初始加载状态配置
 * @returns {Object} 加载状态管理器实例
 */
export function useLoadingState(initialStates = {}) {
  // 如果传入数组，转换为对象格式
  const states = Array.isArray(initialStates)
    ? initialStates.reduce((acc, key) => ({ ...acc, [key]: false }), {})
    : { ...initialStates }

  // 使用 reactive 使对象响应式
  const loadingStates = reactive(states)
  
  // 全局加载状态
  const globalLoading = ref(false)

  // 计算属性

  /**
   * 是否有任何加载状态为 true
   */
  const isLoading = computed(() => {
    return globalLoading.value || Object.values(loadingStates).some(Boolean)
  })

  /**
   * 当前活跃的加载状态列表
   */
  const activeLoadingStates = computed(() => {
    return Object.entries(loadingStates)
      .filter(([, value]) => value)
      .map(([key]) => key)
  })

  /**
   * 活跃的加载状态数量
   */
  const activeLoadingCount = computed(() => {
    return activeLoadingStates.value.length
  })

  // 方法

  /**
   * 设置特定加载状态
   * @param {string} key - 状态键名
   * @param {boolean} value - 状态值
   */
  const setLoading = (key, value) => {
    if (key in loadingStates) {
      loadingStates[key] = value
    } else {
      console.warn(`[useLoadingState] 未知的加载状态: ${key}`)
      // 动态添加新状态
      loadingStates[key] = value
    }
  }

  /**
   * 获取特定加载状态
   * @param {string} key - 状态键名
   * @returns {boolean} 加载状态值
   */
  const getLoading = (key) => {
    return loadingStates[key] || false
  }

  /**
   * 设置全局加载状态
   * @param {boolean} value - 状态值
   */
  const setGlobalLoading = (value) => {
    globalLoading.value = value
  }

  /**
   * 开始加载
   * @param {string|string[]} keys - 状态键名或键名数组
   */
  const startLoading = (keys) => {
    const keyArray = Array.isArray(keys) ? keys : [keys]
    keyArray.forEach(key => setLoading(key, true))
  }

  /**
   * 停止加载
   * @param {string|string[]} keys - 状态键名或键名数组
   */
  const stopLoading = (keys) => {
    const keyArray = Array.isArray(keys) ? keys : [keys]
    keyArray.forEach(key => setLoading(key, false))
  }

  /**
   * 重置所有加载状态
   */
  const resetAll = () => {
    Object.keys(loadingStates).forEach(key => {
      loadingStates[key] = false
    })
    globalLoading.value = false
  }

  /**
   * 重置特定的加载状态
   * @param {string|string[]} keys - 状态键名或键名数组
   */
  const reset = (keys) => {
    const keyArray = Array.isArray(keys) ? keys : [keys]
    keyArray.forEach(key => {
      if (key in loadingStates) {
        loadingStates[key] = false
      }
    })
  }

  /**
   * 添加新的加载状态
   * @param {string} key - 状态键名
   * @param {boolean} initialValue - 初始值，默认 false
   */
  const addState = (key, initialValue = false) => {
    if (!(key in loadingStates)) {
      loadingStates[key] = initialValue
    }
  }

  /**
   * 移除加载状态
   * @param {string} key - 状态键名
   */
  const removeState = (key) => {
    if (key in loadingStates) {
      delete loadingStates[key]
    }
  }

  /**
   * 包装异步函数，自动管理加载状态
   * @param {Function} asyncFn - 异步函数
   * @param {string|string[]} loadingKeys - 要管理的加载状态键
   * @param {Object} options - 配置选项
   * @returns {Function} 包装后的函数
   */
  const withLoading = (asyncFn, loadingKeys, options = {}) => {
    const {
      useGlobal = false,
      onError = null,
      finally: finallyCallback = null
    } = options

    return async (...args) => {
      try {
        if (useGlobal) {
          setGlobalLoading(true)
        } else {
          startLoading(loadingKeys)
        }

        return await asyncFn(...args)
      } catch (error) {
        if (onError) {
          onError(error)
        } else {
          throw error
        }
      } finally {
        if (useGlobal) {
          setGlobalLoading(false)
        } else {
          stopLoading(loadingKeys)
        }

        if (finallyCallback) {
          finallyCallback()
        }
      }
    }
  }

  /**
   * 创建一个加载状态追踪器
   * @param {string} key - 状态键名
   * @returns {Object} 包含 start、stop、toggle 和 isLoading 的对象
   */
  const createTracker = (key) => {
    // 确保状态存在
    if (!(key in loadingStates)) {
      addState(key)
    }

    return {
      /**
       * 开始加载
       */
      start: () => setLoading(key, true),
      
      /**
       * 停止加载
       */
      stop: () => setLoading(key, false),
      
      /**
       * 切换加载状态
       */
      toggle: () => setLoading(key, !loadingStates[key]),
      
      /**
       * 当前加载状态
       */
      isLoading: computed(() => loadingStates[key])
    }
  }

  /**
   * 批量设置多个加载状态
   * @param {Object} states - 状态对象 { key1: true, key2: false, ... }
   */
  const setBatch = (states) => {
    Object.entries(states).forEach(([key, value]) => {
      setLoading(key, value)
    })
  }

  /**
   * 获取所有加载状态的快照
   * @returns {Object} 加载状态快照
   */
  const getSnapshot = () => {
    return {
      global: globalLoading.value,
      states: { ...loadingStates },
      isLoading: isLoading.value,
      activeStates: activeLoadingStates.value
    }
  }

  return {
    // 状态
    loadingStates,
    globalLoading,
    
    // 计算属性
    isLoading,
    activeLoadingStates,
    activeLoadingCount,
    
    // 方法
    setLoading,
    getLoading,
    setGlobalLoading,
    startLoading,
    stopLoading,
    resetAll,
    reset,
    addState,
    removeState,
    withLoading,
    createTracker,
    setBatch,
    getSnapshot
  }
}

/**
 * 创建全局加载状态管理器（单例模式）
 */
let globalLoadingStateManager = null

export function useGlobalLoadingState() {
  if (!globalLoadingStateManager) {
    globalLoadingStateManager = useLoadingState({
      global: false,
      api: false,
      services: false,
      tools: false,
      agents: false,
      dashboard: false,
      page: false
    })
  }
  return globalLoadingStateManager
}

/**
 * 重置全局加载状态管理器
 */
export function resetGlobalLoadingState() {
  if (globalLoadingStateManager) {
    globalLoadingStateManager.resetAll()
    globalLoadingStateManager = null
  }
}

/**
 * 预定义的加载状态键
 */
export const LOADING_KEYS = {
  GLOBAL: 'global',
  API: 'api',
  SERVICES: 'services',
  TOOLS: 'tools',
  AGENTS: 'agents',
  DASHBOARD: 'dashboard',
  PAGE: 'page',
  ADDING: 'adding',
  UPDATING: 'updating',
  DELETING: 'deleting',
  FETCHING: 'fetching',
  SUBMITTING: 'submitting',
  CHECKING: 'checking',
  HEALTH: 'health'
}

