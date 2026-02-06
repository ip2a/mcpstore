export const DEFAULT_PLACEHOLDER = '-'

const parseTimestamp = (value) => {
  if (value === null || value === undefined || value === '') return null
  const numeric = Number(value)
  if (!Number.isNaN(numeric)) {
    if (numeric > 1e12) return numeric // 毫秒时间戳
    if (numeric > 1e6) return numeric // 兼容秒精度 * 1e6 场景
    return null
  }
  const parsed = Date.parse(value)
  return Number.isNaN(parsed) ? null : parsed
}

export const normalizeSeconds = (value, nowMs = Date.now()) => {
  if (value === null || value === undefined || value === '') return null
  const numeric = Number(value)
  if (!Number.isNaN(numeric)) {
    // 如果是未来时间戳
    if (numeric > 1e12) return Math.max(0, (numeric - nowMs) / 1000)
    if (numeric > 1e6) return Math.max(0, (numeric - nowMs) / 1000)
    return Math.max(0, numeric)
  }
  const parsed = parseTimestamp(value)
  if (parsed === null) return null
  return Math.max(0, (parsed - nowMs) / 1000)
}

export const formatRemaining = (durationOrDeadline, fallbackDeadline, nowMs = Date.now()) => {
  const seconds = normalizeSeconds(durationOrDeadline ?? fallbackDeadline, nowMs)
  if (seconds === null) return DEFAULT_PLACEHOLDER
  if (seconds <= 0) return '已到期'
  if (seconds < 60) return `${Math.round(seconds)} 秒`
  if (seconds < 3600) return `${Math.round(seconds / 60)} 分钟`
  return `${Math.round(seconds / 3600)} 小时`
}

export const formatAbsoluteTime = (value) => {
  const parsed = parseTimestamp(value)
  if (parsed === null) return DEFAULT_PLACEHOLDER
  return new Date(parsed).toLocaleString()
}

export const formatLatency = (value) => {
  if (value === null || value === undefined || value === '') return DEFAULT_PLACEHOLDER
  const num = Number(value)
  if (Number.isNaN(num)) return DEFAULT_PLACEHOLDER
  const fixed = num >= 100 ? Math.round(num) : Number(num.toFixed(1))
  return `${fixed} ms`
}

export const formatErrorRate = (value) => {
  if (value === null || value === undefined || value === '') return DEFAULT_PLACEHOLDER
  const num = Number(value)
  if (Number.isNaN(num)) return DEFAULT_PLACEHOLDER
  const ratio = num > 1 ? num / 100 : num
  const percent = ratio * 100
  return `${percent.toFixed(percent >= 10 ? 1 : 2)}%`
}

export const errorRateLevel = (value) => {
  const num = Number(value)
  if (Number.isNaN(num)) return 'neutral'
  const ratio = num > 1 ? num / 100 : num
  if (ratio >= 0.2) return 'danger'
  if (ratio >= 0.05) return 'warn'
  return 'ok'
}

export const formatSampleSize = (value) => {
  if (value === null || value === undefined) return DEFAULT_PLACEHOLDER
  const num = Number(value)
  if (Number.isNaN(num)) return DEFAULT_PLACEHOLDER
  return num.toLocaleString('zh-CN')
}
