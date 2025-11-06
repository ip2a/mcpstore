/**
 * 常量定义
 */

// API相关常量
export const API_CONFIG = {
  BASE_URL: import.meta.env.VITE_API_BASE_URL,
  TIMEOUT: parseInt(import.meta.env.VITE_API_TIMEOUT) || 30000,
  RETRY_TIMES: 3,
  RETRY_DELAY: 1000
}

// 存储键名
export const STORAGE_KEYS = {
  TOKEN: 'mcpstore-token',
  USER_INFO: 'mcpstore-user',
  THEME: 'mcpstore-theme',
  LANGUAGE: 'mcpstore-language',
  SIDEBAR_COLLAPSE: 'mcpstore-collapse',
  RECENT_SERVICES: 'mcpstore-recent-services',
  RECENT_TOOLS: 'mcpstore-recent-tools'
}

// 主题配置
export const THEME_CONFIG = {
  LIGHT: 'light',
  DARK: 'dark',
  AUTO: 'auto'
}

// 语言配置
export const LANGUAGE_CONFIG = {
  ZH_CN: 'zh-CN',
  EN_US: 'en-US'
}

// 🔧 服务生命周期状态 - 7状态系统（2025-07-31更新）
export const SERVICE_STATUS = {
  INITIALIZING: 'initializing',    // 初始化中
  HEALTHY: 'healthy',              // 健康
  WARNING: 'warning',              // 警告（响应慢但正常）
  RECONNECTING: 'reconnecting',    // 重连中
  UNREACHABLE: 'unreachable',      // 不可达
  DISCONNECTING: 'disconnecting',  // 断开连接中
  DISCONNECTED: 'disconnected'     // 已断开
}

// 🔧 服务状态映射 - 7状态系统
export const SERVICE_STATUS_MAP = {
  [SERVICE_STATUS.INITIALIZING]: '初始化中',
  [SERVICE_STATUS.HEALTHY]: '健康',
  [SERVICE_STATUS.WARNING]: '警告',
  [SERVICE_STATUS.RECONNECTING]: '重连中',
  [SERVICE_STATUS.UNREACHABLE]: '不可达',
  [SERVICE_STATUS.DISCONNECTING]: '断开中',
  [SERVICE_STATUS.DISCONNECTED]: '已断开'
}

// 🔧 服务状态颜色 - 7状态系统
export const SERVICE_STATUS_COLORS = {
  [SERVICE_STATUS.INITIALIZING]: 'primary',
  [SERVICE_STATUS.HEALTHY]: 'success',
  [SERVICE_STATUS.WARNING]: 'warning',
  [SERVICE_STATUS.RECONNECTING]: 'primary',
  [SERVICE_STATUS.UNREACHABLE]: 'danger',
  [SERVICE_STATUS.DISCONNECTING]: 'warning',
  [SERVICE_STATUS.DISCONNECTED]: 'info'
}

// 工具执行状态
export const TOOL_EXECUTION_STATUS = {
  PENDING: 'pending',
  RUNNING: 'running',
  SUCCESS: 'success',
  FAILED: 'failed',
  ERROR: 'failed',
  TIMEOUT: 'timeout'
}

// 工具执行状态映射
export const TOOL_EXECUTION_STATUS_MAP = {
  [TOOL_EXECUTION_STATUS.PENDING]: '等待中',
  [TOOL_EXECUTION_STATUS.RUNNING]: '执行中',
  [TOOL_EXECUTION_STATUS.SUCCESS]: '成功',
  [TOOL_EXECUTION_STATUS.FAILED]: '失败',
  [TOOL_EXECUTION_STATUS.ERROR]: '失败',
  [TOOL_EXECUTION_STATUS.TIMEOUT]: '超时'
}

// 工具执行状态颜色
export const TOOL_EXECUTION_STATUS_COLORS = {
  [TOOL_EXECUTION_STATUS.PENDING]: 'info',
  [TOOL_EXECUTION_STATUS.RUNNING]: 'warning',
  [TOOL_EXECUTION_STATUS.SUCCESS]: 'success',
  [TOOL_EXECUTION_STATUS.FAILED]: 'danger',
  [TOOL_EXECUTION_STATUS.ERROR]: 'danger',
  [TOOL_EXECUTION_STATUS.TIMEOUT]: 'danger'
}

// Agent状态
export const AGENT_STATUS = {
  ACTIVE: 'active',
  INACTIVE: 'inactive',
  BUSY: 'busy',
  ERROR: 'error'
}

// Agent状态映射
export const AGENT_STATUS_MAP = {
  [AGENT_STATUS.ACTIVE]: '活跃',
  [AGENT_STATUS.INACTIVE]: '非活跃',
  [AGENT_STATUS.BUSY]: '忙碌',
  [AGENT_STATUS.ERROR]: '错误'
}

// Agent状态颜色
export const AGENT_STATUS_COLORS = {
  [AGENT_STATUS.ACTIVE]: 'success',
  [AGENT_STATUS.INACTIVE]: 'info',
  [AGENT_STATUS.BUSY]: 'warning',
  [AGENT_STATUS.ERROR]: 'danger'
}

// 文件类型
export const FILE_TYPES = {
  IMAGE: ['jpg', 'jpeg', 'png', 'gif', 'bmp', 'webp', 'svg'],
  DOCUMENT: ['pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'txt'],
  ARCHIVE: ['zip', 'rar', '7z', 'tar', 'gz'],
  CODE: ['js', 'ts', 'vue', 'html', 'css', 'scss', 'json', 'xml', 'py', 'java', 'cpp', 'c'],
  VIDEO: ['mp4', 'avi', 'mov', 'wmv', 'flv', 'mkv'],
  AUDIO: ['mp3', 'wav', 'flac', 'aac', 'ogg']
}

// 文件大小限制（字节）
export const FILE_SIZE_LIMITS = {
  IMAGE: 10 * 1024 * 1024, // 10MB
  DOCUMENT: 50 * 1024 * 1024, // 50MB
  ARCHIVE: 100 * 1024 * 1024, // 100MB
  CODE: 5 * 1024 * 1024, // 5MB
  VIDEO: 500 * 1024 * 1024, // 500MB
  AUDIO: 100 * 1024 * 1024 // 100MB
}

// 分页配置
export const PAGINATION_CONFIG = {
  PAGE_SIZE: 20,
  PAGE_SIZES: [10, 20, 50, 100],
  LAYOUT: 'total, sizes, prev, pager, next, jumper'
}

// 表格配置
export const TABLE_CONFIG = {
  STRIPE: true,
  BORDER: true,
  SIZE: 'default',
  HIGHLIGHT_CURRENT_ROW: true,
  EMPTY_TEXT: '暂无数据'
}

// 消息配置
export const MESSAGE_CONFIG = {
  DURATION: 3000,
  SHOW_CLOSE: true,
  CENTER: false
}

// 通知配置
export const NOTIFICATION_CONFIG = {
  DURATION: 4500,
  POSITION: 'top-right'
}

// 加载配置
export const LOADING_CONFIG = {
  TEXT: '加载中...',
  SPINNER: 'el-icon-loading',
  BACKGROUND: 'rgba(0, 0, 0, 0.7)'
}

// 对话框配置
export const DIALOG_CONFIG = {
  WIDTH: '50%',
  TOP: '15vh',
  MODAL: true,
  MODAL_APPEND_TO_BODY: true,
  APPEND_TO_BODY: false,
  LOCK_SCROLL: true,
  CUSTOM_CLASS: '',
  CLOSE_ON_CLICK_MODAL: true,
  CLOSE_ON_PRESS_ESCAPE: true,
  SHOW_CLOSE: true
}

// 抽屉配置
export const DRAWER_CONFIG = {
  SIZE: '30%',
  DIRECTION: 'rtl',
  MODAL: true,
  MODAL_APPEND_TO_BODY: true,
  APPEND_TO_BODY: false,
  LOCK_SCROLL: true,
  CLOSE_ON_PRESS_ESCAPE: true,
  SHOW_CLOSE: true
}

// 表单验证规则
export const FORM_RULES = {
  REQUIRED: { required: true, message: '此项为必填项', trigger: 'blur' },
  EMAIL: { type: 'email', message: '请输入正确的邮箱地址', trigger: 'blur' },
  URL: { type: 'url', message: '请输入正确的URL地址', trigger: 'blur' },
  NUMBER: { type: 'number', message: '请输入数字', trigger: 'blur' },
  INTEGER: { type: 'integer', message: '请输入整数', trigger: 'blur' },
  PHONE: { pattern: /^1[3-9]\d{9}$/, message: '请输入正确的手机号', trigger: 'blur' },
  PASSWORD: { min: 6, max: 20, message: '密码长度为6-20位', trigger: 'blur' }
}

// 正则表达式
export const REGEX_PATTERNS = {
  EMAIL: /^[^\s@]+@[^\s@]+\.[^\s@]+$/,
  PHONE: /^1[3-9]\d{9}$/,
  ID_CARD: /^[1-9]\d{5}(18|19|20)\d{2}((0[1-9])|(1[0-2]))(([0-2][1-9])|10|20|30|31)\d{3}[0-9Xx]$/,
  URL: /^https?:\/\/(([a-zA-Z0-9_-])+(\.)?)*(:\d+)?(\/((\.)?(\?)?=?&?[a-zA-Z0-9_-](\?)?)*)*$/i,
  IP: /^((25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$/,
  USERNAME: /^[a-zA-Z_][a-zA-Z0-9_]{3,19}$/,
  CHINESE_NAME: /^[\u4e00-\u9fa5]{2,10}$/,
  PASSWORD: /^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)[a-zA-Z\d@$!%*?&]{8,}$/
}

// 错误码映射
export const ERROR_CODE_MAP = {
  400: '请求参数错误',
  401: '未授权访问',
  403: '禁止访问',
  404: '资源不存在',
  405: '请求方法不允许',
  408: '请求超时',
  409: '资源冲突',
  422: '请求参数验证失败',
  429: '请求过于频繁',
  500: '服务器内部错误',
  502: '网关错误',
  503: '服务不可用',
  504: '网关超时'
}

// 成功码映射
export const SUCCESS_CODE_MAP = {
  200: '请求成功',
  201: '创建成功',
  202: '请求已接受',
  204: '删除成功'
}

// 默认头像
export const DEFAULT_AVATAR = 'data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNDAiIGhlaWdodD0iNDAiIHZpZXdCb3g9IjAgMCA0MCA0MCIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPGNpcmNsZSBjeD0iMjAiIGN5PSIyMCIgcj0iMjAiIGZpbGw9IiNGNUY1RjUiLz4KPHBhdGggZD0iTTIwIDIwQzIzLjMxMzcgMjAgMjYgMTcuMzEzNyAyNiAxNEMyNiAxMC42ODYzIDIzLjMxMzcgOCAyMCA4QzE2LjY4NjMgOCAxNCA0LjY4NjMgMTQgMTRDMTQgMTcuMzEzNyAxNi42ODYzIDIwIDIwIDIwWiIgZmlsbD0iI0NDQ0NDQyIvPgo8cGF0aCBkPSJNMjAgMjJDMTQuNDc3MiAyMiAxMCAyNi40NzcyIDEwIDMyVjM0QzEwIDM1LjEwNDYgMTAuODk1NCAzNiAxMiAzNkgyOEMyOS4xMDQ2IDM2IDMwIDM1LjEwNDYgMzAgMzRWMzJDMzAgMjYuNDc3MiAyNS41MjI4IDIyIDIwIDIyWiIgZmlsbD0iI0NDQ0NDQyIvPgo8L3N2Zz4K'

// 系统信息
export const SYSTEM_INFO = {
  NAME: 'MCPStore',
  VERSION: import.meta.env.VITE_APP_VERSION || '1.4.1',
  DESCRIPTION: import.meta.env.VITE_APP_DESCRIPTION || 'MCP工具服务商店',
  AUTHOR: 'MCPStore Team',
  COPYRIGHT: `© ${new Date().getFullYear()} MCPStore. All rights reserved.`
}
