/**
 * 工具函数入口文件
 */

export * from './format'
export * from './validate'
export * from './constants'

/**
 * 深拷贝对象
 * 递归复制对象的所有属性，支持 Date、Array 和普通对象
 * 
 * @param {any} obj - 要拷贝的对象
 * @returns {any} 拷贝后的对象
 * 
 * @example
 * const original = { a: 1, b: { c: 2 } }
 * const cloned = deepClone(original)
 * cloned.b.c = 3
 * console.log(original.b.c) // 输出: 2
 */
export function deepClone(obj) {
  if (obj === null || typeof obj !== 'object') return obj
  if (obj instanceof Date) return new Date(obj.getTime())
  if (obj instanceof Array) return obj.map(item => deepClone(item))
  if (typeof obj === 'object') {
    const clonedObj = {}
    for (const key in obj) {
      if (obj.hasOwnProperty(key)) {
        clonedObj[key] = deepClone(obj[key])
      }
    }
    return clonedObj
  }
}

/**
 * 防抖函数
 * 在事件触发 n 秒后才执行，如果在 n 秒内又触发了事件，则重新计时
 * 
 * @param {Function} func - 要防抖的函数
 * @param {number} wait - 等待时间（毫秒）
 * @returns {Function} 防抖后的函数
 * 
 * @example
 * const debouncedSearch = debounce((query) => {
 *   console.log('Searching for:', query)
 * }, 300)
 * 
 * // 多次快速调用只会执行最后一次
 * debouncedSearch('a')
 * debouncedSearch('ab')
 * debouncedSearch('abc') // 只有这次会在 300ms 后执行
 */
export function debounce(func, wait) {
  let timeout
  return function executedFunction(...args) {
    const later = () => {
      clearTimeout(timeout)
      func(...args)
    }
    clearTimeout(timeout)
    timeout = setTimeout(later, wait)
  }
}

/**
 * 节流函数
 * 限制函数在指定时间内只能执行一次
 * 
 * @param {Function} func - 要节流的函数
 * @param {number} limit - 时间限制（毫秒）
 * @returns {Function} 节流后的函数
 * 
 * @example
 * const throttledScroll = throttle(() => {
 *   console.log('Scroll event handled')
 * }, 1000)
 * 
 * window.addEventListener('scroll', throttledScroll)
 * // 无论滚动多快，每秒最多执行一次
 */
export function throttle(func, limit) {
  let inThrottle
  return function executedFunction(...args) {
    if (!inThrottle) {
      func.apply(this, args)
      inThrottle = true
      setTimeout(() => inThrottle = false, limit)
    }
  }
}

/**
 * 生成UUID
 * @returns {string} UUID字符串
 */
export function generateUUID() {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
    const r = Math.random() * 16 | 0
    const v = c === 'x' ? r : (r & 0x3 | 0x8)
    return v.toString(16)
  })
}

/**
 * 获取文件扩展名
 * @param {string} filename 文件名
 * @returns {string} 扩展名
 */
export function getFileExtension(filename) {
  return filename.slice((filename.lastIndexOf('.') - 1 >>> 0) + 2)
}

/**
 * 下载文件
 * @param {string} url 文件URL
 * @param {string} filename 文件名
 */
export function downloadFile(url, filename) {
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
}

/**
 * 复制文本到剪贴板
 * 优先使用现代 Clipboard API，如果不支持则降级使用 execCommand
 * 
 * @param {string} text - 要复制的文本
 * @returns {Promise<boolean>} 是否成功复制
 * 
 * @example
 * const success = await copyToClipboard('Hello World')
 * if (success) {
 *   console.log('复制成功')
 * } else {
 *   console.log('复制失败')
 * }
 */
export async function copyToClipboard(text) {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch (err) {
    // 降级方案
    const textArea = document.createElement('textarea')
    textArea.value = text
    document.body.appendChild(textArea)
    textArea.select()
    try {
      document.execCommand('copy')
      return true
    } catch (err) {
      return false
    } finally {
      document.body.removeChild(textArea)
    }
  }
}

/**
 * 获取浏览器信息
 * @returns {object} 浏览器信息
 */
export function getBrowserInfo() {
  const ua = navigator.userAgent
  const isChrome = /Chrome/.test(ua) && /Google Inc/.test(navigator.vendor)
  const isFirefox = /Firefox/.test(ua)
  const isSafari = /Safari/.test(ua) && /Apple Computer/.test(navigator.vendor)
  const isEdge = /Edg/.test(ua)
  
  return {
    isChrome,
    isFirefox,
    isSafari,
    isEdge,
    userAgent: ua
  }
}

/**
 * 检查是否为移动设备
 * @returns {boolean} 是否为移动设备
 */
export function isMobile() {
  return /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(navigator.userAgent)
}

/**
 * 获取URL参数
 * @param {string} name 参数名
 * @returns {string|null} 参数值
 */
export function getUrlParam(name) {
  const urlParams = new URLSearchParams(window.location.search)
  return urlParams.get(name)
}

/**
 * 设置URL参数
 * @param {string} name 参数名
 * @param {string} value 参数值
 */
export function setUrlParam(name, value) {
  const url = new URL(window.location)
  url.searchParams.set(name, value)
  window.history.pushState({}, '', url)
}
