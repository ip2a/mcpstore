/**
 * 验证工具函数
 */

/**
 * 验证邮箱
 * @param {string} email 邮箱地址
 * @returns {boolean} 是否有效
 */
export function validateEmail(email) {
  const regex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
  return regex.test(email)
}

/**
 * 验证手机号
 * @param {string} phone 手机号
 * @returns {boolean} 是否有效
 */
export function validatePhone(phone) {
  const regex = /^1[3-9]\d{9}$/
  return regex.test(phone)
}

/**
 * 验证身份证号
 * @param {string} idCard 身份证号
 * @returns {boolean} 是否有效
 */
export function validateIdCard(idCard) {
  if (!idCard || idCard.length !== 18) return false
  
  const regex = /^[1-9]\d{5}(18|19|20)\d{2}((0[1-9])|(1[0-2]))(([0-2][1-9])|10|20|30|31)\d{3}[0-9Xx]$/
  if (!regex.test(idCard)) return false
  
  // 验证校验码
  const weights = [7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2]
  const checkCodes = ['1', '0', 'X', '9', '8', '7', '6', '5', '4', '3', '2']
  
  let sum = 0
  for (let i = 0; i < 17; i++) {
    sum += parseInt(idCard[i]) * weights[i]
  }
  
  const checkCode = checkCodes[sum % 11]
  return checkCode === idCard[17].toUpperCase()
}

/**
 * 验证URL
 * @param {string} url URL地址
 * @returns {boolean} 是否有效
 */
export function validateURL(url) {
  try {
    new URL(url)
    return true
  } catch {
    return false
  }
}

/**
 * 验证IP地址
 * @param {string} ip IP地址
 * @returns {boolean} 是否有效
 */
export function validateIP(ip) {
  const regex = /^((25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$/
  return regex.test(ip)
}

/**
 * 验证端口号
 * @param {string|number} port 端口号
 * @returns {boolean} 是否有效
 */
export function validatePort(port) {
  const num = parseInt(port)
  return !isNaN(num) && num >= 1 && num <= 65535
}

/**
 * 验证密码强度
 * @param {string} password 密码
 * @returns {object} 验证结果
 */
export function validatePassword(password) {
  if (!password) {
    return { valid: false, strength: 0, message: '密码不能为空' }
  }
  
  let strength = 0
  const checks = {
    length: password.length >= 8,
    lowercase: /[a-z]/.test(password),
    uppercase: /[A-Z]/.test(password),
    number: /\d/.test(password),
    special: /[!@#$%^&*(),.?":{}|<>]/.test(password)
  }
  
  strength += checks.length ? 1 : 0
  strength += checks.lowercase ? 1 : 0
  strength += checks.uppercase ? 1 : 0
  strength += checks.number ? 1 : 0
  strength += checks.special ? 1 : 0
  
  let message = ''
  if (strength < 3) {
    message = '密码强度较弱'
  } else if (strength < 4) {
    message = '密码强度中等'
  } else {
    message = '密码强度较强'
  }
  
  return {
    valid: strength >= 3,
    strength,
    message,
    checks
  }
}

/**
 * 验证用户名
 * @param {string} username 用户名
 * @returns {boolean} 是否有效
 */
export function validateUsername(username) {
  if (!username) return false
  
  // 4-20位，字母、数字、下划线，不能以数字开头
  const regex = /^[a-zA-Z_][a-zA-Z0-9_]{3,19}$/
  return regex.test(username)
}

/**
 * 验证中文姓名
 * @param {string} name 姓名
 * @returns {boolean} 是否有效
 */
export function validateChineseName(name) {
  if (!name) return false
  
  const regex = /^[\u4e00-\u9fa5]{2,10}$/
  return regex.test(name)
}

/**
 * 验证银行卡号
 * @param {string} cardNumber 银行卡号
 * @returns {boolean} 是否有效
 */
export function validateBankCard(cardNumber) {
  if (!cardNumber) return false
  
  const cleaned = cardNumber.replace(/\D/g, '')
  if (cleaned.length < 16 || cleaned.length > 19) return false
  
  // Luhn算法验证
  let sum = 0
  let isEven = false
  
  for (let i = cleaned.length - 1; i >= 0; i--) {
    let digit = parseInt(cleaned[i])
    
    if (isEven) {
      digit *= 2
      if (digit > 9) {
        digit -= 9
      }
    }
    
    sum += digit
    isEven = !isEven
  }
  
  return sum % 10 === 0
}

/**
 * 验证JSON格式
 * @param {string} jsonString JSON字符串
 * @returns {boolean} 是否有效
 */
export function validateJSON(jsonString) {
  try {
    JSON.parse(jsonString)
    return true
  } catch {
    return false
  }
}

/**
 * 验证正整数
 * @param {string|number} value 值
 * @returns {boolean} 是否有效
 */
export function validatePositiveInteger(value) {
  const num = parseInt(value)
  return !isNaN(num) && num > 0 && num.toString() === value.toString()
}

/**
 * 验证非负数
 * @param {string|number} value 值
 * @returns {boolean} 是否有效
 */
export function validateNonNegativeNumber(value) {
  const num = parseFloat(value)
  return !isNaN(num) && num >= 0
}

/**
 * 验证数字范围
 * @param {string|number} value 值
 * @param {number} min 最小值
 * @param {number} max 最大值
 * @returns {boolean} 是否有效
 */
export function validateNumberRange(value, min, max) {
  const num = parseFloat(value)
  return !isNaN(num) && num >= min && num <= max
}

/**
 * 验证字符串长度
 * @param {string} str 字符串
 * @param {number} min 最小长度
 * @param {number} max 最大长度
 * @returns {boolean} 是否有效
 */
export function validateStringLength(str, min = 0, max = Infinity) {
  if (typeof str !== 'string') return false
  return str.length >= min && str.length <= max
}

/**
 * 验证文件类型
 * @param {File} file 文件对象
 * @param {Array} allowedTypes 允许的类型
 * @returns {boolean} 是否有效
 */
export function validateFileType(file, allowedTypes = []) {
  if (!file || !allowedTypes.length) return false
  
  return allowedTypes.some(type => {
    if (type.startsWith('.')) {
      return file.name.toLowerCase().endsWith(type.toLowerCase())
    } else {
      return file.type.toLowerCase().includes(type.toLowerCase())
    }
  })
}

/**
 * 验证文件大小
 * @param {File} file 文件对象
 * @param {number} maxSize 最大大小（字节）
 * @returns {boolean} 是否有效
 */
export function validateFileSize(file, maxSize) {
  if (!file) return false
  return file.size <= maxSize
}

/**
 * 验证日期格式
 * @param {string} dateString 日期字符串
 * @param {string} format 日期格式
 * @returns {boolean} 是否有效
 */
export function validateDateFormat(dateString, format = 'YYYY-MM-DD') {
  if (!dateString) return false
  
  const date = new Date(dateString)
  return !isNaN(date.getTime())
}

/**
 * 验证日期范围
 * @param {string|Date} date 日期
 * @param {string|Date} minDate 最小日期
 * @param {string|Date} maxDate 最大日期
 * @returns {boolean} 是否有效
 */
export function validateDateRange(date, minDate, maxDate) {
  const d = new Date(date)
  const min = new Date(minDate)
  const max = new Date(maxDate)
  
  if (isNaN(d.getTime())) return false
  if (minDate && !isNaN(min.getTime()) && d < min) return false
  if (maxDate && !isNaN(max.getTime()) && d > max) return false
  
  return true
}
