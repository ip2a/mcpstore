// 服务健康状态与展示元数据
export const SERVICE_STATUS = {
  INIT: 'init',
  STARTUP: 'startup',
  READY: 'ready',
  HEALTHY: 'healthy',
  DEGRADED: 'degraded',
  CIRCUIT_OPEN: 'circuit_open',
  HALF_OPEN: 'half_open',
  DISCONNECTED: 'disconnected'
}

const STATUS_META = {
  [SERVICE_STATUS.INIT]: {
    text: '初始化',
    tone: 'info',
    className: 'is-init',
    badgeClass: 'mcp-status-init'
  },
  [SERVICE_STATUS.STARTUP]: {
    text: '启动中',
    tone: 'info',
    className: 'is-startup',
    badgeClass: 'mcp-status-startup'
  },
  [SERVICE_STATUS.READY]: {
    text: '已就绪',
    tone: 'ready',
    className: 'is-ready',
    badgeClass: 'mcp-status-ready'
  },
  [SERVICE_STATUS.HEALTHY]: {
    text: '健康',
    tone: 'success',
    className: 'is-healthy',
    badgeClass: 'mcp-status-healthy'
  },
  [SERVICE_STATUS.DEGRADED]: {
    text: '性能下降',
    tone: 'warning',
    className: 'is-degraded',
    badgeClass: 'mcp-status-degraded'
  },
  [SERVICE_STATUS.CIRCUIT_OPEN]: {
    text: '已熔断',
    tone: 'danger',
    className: 'is-circuit-open',
    badgeClass: 'mcp-status-circuit-open'
  },
  [SERVICE_STATUS.HALF_OPEN]: {
    text: '半开试探',
    tone: 'warning',
    className: 'is-half-open',
    badgeClass: 'mcp-status-half-open'
  },
  [SERVICE_STATUS.DISCONNECTED]: {
    text: '已断连',
    tone: 'muted',
    className: 'is-disconnected',
    badgeClass: 'mcp-status-disconnected'
  }
}

export const STATUS_ORDER = [
  SERVICE_STATUS.INIT,
  SERVICE_STATUS.STARTUP,
  SERVICE_STATUS.READY,
  SERVICE_STATUS.HEALTHY,
  SERVICE_STATUS.DEGRADED,
  SERVICE_STATUS.HALF_OPEN,
  SERVICE_STATUS.CIRCUIT_OPEN,
  SERVICE_STATUS.DISCONNECTED
]

export const normalizeStatus = (status) => {
  if (!status) return 'unknown'
  return String(status).toLowerCase()
}

export const getStatusMeta = (status) => {
  const key = normalizeStatus(status)
  const meta = STATUS_META[key]
  if (meta) return meta
  return {
    text: status || '未知',
    tone: 'muted',
    className: 'is-unknown',
    badgeClass: 'mcp-status-unknown'
  }
}

export const formatStatusText = (status) => getStatusMeta(status).text

export const isServiceAvailable = (status) => {
  const key = normalizeStatus(status)
  return ![
    SERVICE_STATUS.CIRCUIT_OPEN,
    SERVICE_STATUS.HALF_OPEN,
    SERVICE_STATUS.DISCONNECTED,
    SERVICE_STATUS.INIT,
    SERVICE_STATUS.STARTUP
  ].includes(key)
}
