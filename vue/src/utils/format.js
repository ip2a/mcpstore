/**
 * 格式化工具函数
 */

/**
 * 格式化日期时间
 * @param {Date|string|number} date 日期
 * @param {string} format 格式化字符串
 * @returns {string} 格式化后的日期字符串
 */
export function formatDateTime(date, format = 'YYYY-MM-DD HH:mm:ss') {
  if (!date) return ''
  
  const d = new Date(date)
  if (isNaN(d.getTime())) return ''
  
  const year = d.getFullYear()
  const month = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  const hours = String(d.getHours()).padStart(2, '0')
  const minutes = String(d.getMinutes()).padStart(2, '0')
  const seconds = String(d.getSeconds()).padStart(2, '0')
  
  return format
    .replace('YYYY', year)
    .replace('MM', month)
    .replace('DD', day)
    .replace('HH', hours)
    .replace('mm', minutes)
    .replace('ss', seconds)
}

/**
 * 格式化相对时间
 * @param {Date|string|number} date 日期
 * @returns {string} 相对时间字符串
 */
export function formatRelativeTime(date) {
  if (!date) return ''
  
  const d = new Date(date)
  if (isNaN(d.getTime())) return ''
  
  const now = new Date()
  const diff = now.getTime() - d.getTime()
  const seconds = Math.floor(diff / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  const days = Math.floor(hours / 24)
  
  if (seconds < 60) return '刚刚'
  if (minutes < 60) return `${minutes}分钟前`
  if (hours < 24) return `${hours}小时前`
  if (days < 7) return `${days}天前`
  
  return formatDateTime(date, 'YYYY-MM-DD')
}

/**
 * 格式化文件大小
 * @param {number} bytes 字节数
 * @param {number} decimals 小数位数
 * @returns {string} 格式化后的文件大小
 */
export function formatFileSize(bytes, decimals = 2) {
  if (bytes === 0) return '0 B'
  
  const k = 1024
  const dm = decimals < 0 ? 0 : decimals
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB']
  
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  
  return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i]
}

/**
 * 格式化数字
 * @param {number} num 数字
 * @param {number} decimals 小数位数
 * @returns {string} 格式化后的数字
 */
export function formatNumber(num, decimals = 0) {
  if (isNaN(num)) return '0'
  
  return Number(num).toLocaleString('zh-CN', {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals
  })
}

/**
 * 格式化百分比
 * @param {number} num 数字
 * @param {number} decimals 小数位数
 * @returns {string} 格式化后的百分比
 */
export function formatPercentage(num, decimals = 1) {
  if (isNaN(num)) return '0%'
  
  return (num * 100).toFixed(decimals) + '%'
}

/**
 * 格式化货币
 * @param {number} amount 金额
 * @param {string} currency 货币符号
 * @returns {string} 格式化后的货币
 */
export function formatCurrency(amount, currency = '¥') {
  if (isNaN(amount)) return currency + '0.00'
  
  return currency + Number(amount).toLocaleString('zh-CN', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2
  })
}

/**
 * 格式化手机号
 * @param {string} phone 手机号
 * @returns {string} 格式化后的手机号
 */
export function formatPhone(phone) {
  if (!phone) return ''
  
  const cleaned = phone.replace(/\D/g, '')
  if (cleaned.length === 11) {
    return cleaned.replace(/(\d{3})(\d{4})(\d{4})/, '$1 $2 $3')
  }
  
  return phone
}

/**
 * 格式化身份证号
 * @param {string} idCard 身份证号
 * @param {boolean} mask 是否遮罩
 * @returns {string} 格式化后的身份证号
 */
export function formatIdCard(idCard, mask = true) {
  if (!idCard) return ''
  
  if (mask && idCard.length === 18) {
    return idCard.replace(/(\d{6})\d{8}(\d{4})/, '$1********$2')
  }
  
  return idCard
}

/**
 * 格式化银行卡号
 * @param {string} cardNumber 银行卡号
 * @param {boolean} mask 是否遮罩
 * @returns {string} 格式化后的银行卡号
 */
export function formatBankCard(cardNumber, mask = true) {
  if (!cardNumber) return ''
  
  const cleaned = cardNumber.replace(/\D/g, '')
  
  if (mask && cleaned.length >= 8) {
    const start = cleaned.slice(0, 4)
    const end = cleaned.slice(-4)
    const middle = '*'.repeat(cleaned.length - 8)
    return `${start} ${middle} ${end}`.replace(/(.{4})/g, '$1 ').trim()
  }
  
  return cleaned.replace(/(.{4})/g, '$1 ').trim()
}

/**
 * 格式化JSON
 * @param {any} obj 对象
 * @param {number} space 缩进空格数
 * @returns {string} 格式化后的JSON字符串
 */
export function formatJSON(obj, space = 2) {
  try {
    return JSON.stringify(obj, null, space)
  } catch (error) {
    return String(obj)
  }
}

/**
 * 格式化URL
 * @param {string} url URL
 * @returns {string} 格式化后的URL
 */
export function formatURL(url) {
  if (!url) return ''
  
  if (!/^https?:\/\//i.test(url)) {
    return 'http://' + url
  }
  
  return url
}

/**
 * 格式化状态文本
 * @param {string|number} status 状态值
 * @param {object} statusMap 状态映射
 * @returns {string} 状态文本
 */
export function formatStatus(status, statusMap = {}) {
  return statusMap[status] || status || '未知'
}

/**
 * 格式化枚举值
 * @param {string|number} value 枚举值
 * @param {Array} enumList 枚举列表
 * @returns {string} 枚举文本
 */
export function formatEnum(value, enumList = []) {
  const item = enumList.find(item => item.value === value)
  return item ? item.label : value || '未知'
}

/**
 * 截断文本
 * @param {string} text 文本
 * @param {number} length 最大长度
 * @param {string} suffix 后缀
 * @returns {string} 截断后的文本
 */
export function truncateText(text, length = 50, suffix = '...') {
  if (!text || text.length <= length) return text || ''
  
  return text.slice(0, length) + suffix
}

/**
 * 高亮关键词
 * @param {string} text 文本
 * @param {string} keyword 关键词
 * @param {string} className CSS类名
 * @returns {string} 高亮后的HTML
 */
export function highlightKeyword(text, keyword, className = 'highlight') {
  if (!text || !keyword) return text || ''
  
  const regex = new RegExp(`(${keyword})`, 'gi')
  return text.replace(regex, `<span class="${className}">$1</span>`)
}
