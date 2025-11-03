/**
 * JSON Schema 工具函数
 * 用于处理工具的输入参数 schema
 */

/**
 * 将 JSON Schema 转换为简洁的文本摘要
 * @param {Object} schema - JSON Schema 对象
 * @returns {string} - 参数摘要文本
 */
export function summarizeInputs(schema) {
  if (!schema || typeof schema !== 'object') return ''
  if (schema.type !== 'object' || !schema.properties) return ''
  
  const required = Array.isArray(schema.required) ? schema.required : []
  const props = schema.properties || {}
  const parts = []
  
  for (const key of Object.keys(props)) {
    const p = props[key] || {}
    const isReq = required.includes(key)
    const type = p.type || 'any'
    const extras = []
    
    if (p.default !== undefined) extras.push(`default:${String(p.default)}`)
    if (typeof p.minimum === 'number') extras.push(`min:${p.minimum}`)
    if (typeof p.maximum === 'number') extras.push(`max:${p.maximum}`)
    
    const extraText = extras.length ? ` (${extras.join(', ')})` : ''
    parts.push(`${key}${isReq ? '*' : ''}: ${type}${extraText}`)
  }
  
  return parts.join(', ')
}

/**
 * 将 JSON Schema 转换为结构化列表
 * @param {Object} schema - JSON Schema 对象
 * @returns {Array} - 参数列表数组
 */
export function schemaToList(schema) {
  const list = []
  
  if (!schema || schema.type !== 'object' || !schema.properties) return list
  
  const required = Array.isArray(schema.required) ? schema.required : []
  const props = schema.properties || {}
  
  for (const key of Object.keys(props)) {
    const p = props[key] || {}
    const type = p.type || 'any'
    const extras = []
    
    if (p.default !== undefined) extras.push(`default:${String(p.default)}`)
    if (typeof p.minimum === 'number') extras.push(`min:${p.minimum}`)
    if (typeof p.maximum === 'number') extras.push(`max:${p.maximum}`)
    
    list.push({
      key,
      required: required.includes(key),
      type,
      extras: extras.join(', ')
    })
  }
  
  return list
}

/**
 * 获取参数数量
 * @param {Object} schema - JSON Schema 对象
 * @returns {number} - 参数数量
 */
export function getParameterCount(schema) {
  if (!schema || !schema.properties) return 0
  return Object.keys(schema.properties).length
}
