export const logger = {
  isEnabled() {
    // 仅在开发环境或明确开启时输出调试信息
    return import.meta.env.DEV || import.meta.env.VITE_ENABLE_CONSOLE_LOG === 'true'
  },
  debug(...args) {
    if (this.isEnabled()) console.log(...args)
  },
  info(...args) {
    if (this.isEnabled()) console.log(...args)
  },
  warn(...args) {
    if (this.isEnabled()) console.warn(...args)
  },
  error(...args) {
    // 错误永远打印
    console.error(...args)
  }
}
