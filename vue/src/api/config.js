/**
 * MCPStore API v1.0.0 配置
 * 统一的 API 配置和常量定义
 */

// API 版本
export const API_VERSION = '1.0.0'
export const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || '/api'
export const API_TIMEOUT_MS = parseInt(import.meta.env.VITE_API_TIMEOUT) || 30000

// API 端点路径
export const API_ENDPOINTS = {
  // Store 级别 API
  STORE: {
    SYNC_SERVICES: '/for_store/sync_services',
    SYNC_STATUS: '/for_store/sync_status',
    LIST_SERVICES: '/for_store/list_services',
    ADD_SERVICE: '/for_store/add_service',
    INIT_SERVICE: '/for_store/init_service',
    DELETE_SERVICE: '/for_store/delete_service/{service_name}',
    LIST_TOOLS: '/for_store/list_tools',
    CALL_TOOL: '/for_store/call_tool',
    TOOL_INFO: '/for_store/tool_info/{tool_name}',
    SERVICE_INFO: '/for_store/service_info/{service_name}',
    SERVICE_STATUS: '/for_store/service_status/{service_name}',
    SERVICE_HEALTH: '/for_store/service_health/{service_name}',
    CHECK_SERVICES: '/for_store/check_services',
    HEALTH: '/for_store/health',
    SHOW_CONFIG: '/for_store/show_config',
    SHOW_MCPJSON: '/for_store/show_mcpjson',
    UPDATE_CONFIG: '/for_store/update_config/{client_id_or_service_name}',
    RESET_CONFIG: '/for_store/reset_config',
    RESET_MCPJSON: '/for_store/reset_mcpjson',

    TOOL_RECORDS: '/for_store/tool_records',
    SYSTEM_RESOURCES: '/for_store/system_resources',
    NETWORK_CHECK: '/for_store/network_check',
    LIST_AGENTS: '/for_store/list_agents',
    LIST_SERVICES_BY_AGENT: '/for_store/list_services_by_agent',
    RESTART_SERVICE: '/for_store/restart_service',
    UPDATE_SERVICE: '/for_store/update_service/{service_name}',
    BATCH_UPDATE_SERVICES: '/for_store/batch_update_services',
    BATCH_DELETE_SERVICES: '/for_store/batch_delete_services',
    BATCH_RESTART_SERVICES: '/for_store/batch_restart_services'
  },

  // Agent 级别 API
  AGENT: {
    ADD_SERVICE: '/for_agent/{agent_id}/add_service',
    LIST_SERVICES: '/for_agent/{agent_id}/list_services',
    INIT_SERVICE: '/for_agent/{agent_id}/init_service',
    DELETE_SERVICE: '/for_agent/{agent_id}/delete_service/{service_name}',
    UPDATE_SERVICE: '/for_agent/{agent_id}/update_service/{service_name}',
    LIST_TOOLS: '/for_agent/{agent_id}/list_tools',
    CALL_TOOL: '/for_agent/{agent_id}/call_tool',
    WAIT_SERVICE: '/for_agent/{agent_id}/wait_service',
    RESTART_SERVICE: '/for_agent/{agent_id}/restart_service',
    SERVICE_INFO: '/for_agent/{agent_id}/service_info/{service_name}',
    SERVICE_STATUS: '/for_agent/{agent_id}/service_status/{service_name}',
    SERVICE_HEALTH: '/for_agent/{agent_id}/service_health/{service_name}',
    SERVICE_HEALTH_DETAILS: '/for_agent/{agent_id}/service_health_details/{service_name}',
    CHECK_SERVICES: '/for_agent/{agent_id}/check_services',
    HEALTH: '/for_agent/{agent_id}/health',
    SHOW_CONFIG: '/for_agent/{agent_id}/show_config',
    SHOW_MCP_CONFIG: '/for_agent/{agent_id}/show_mcpconfig',
    GET_JSON_CONFIG: '/for_agent/{agent_id}/get_json_config',
    UPDATE_CONFIG: '/for_agent/{agent_id}/update_config/{client_id_or_service_name}',
    DELETE_CONFIG: '/for_agent/{agent_id}/delete_config/{client_id_or_service_name}',
    RESET_CONFIG: '/for_agent/{agent_id}/reset_config',
    RESET_CLIENT_SERVICES: '/for_agent/{agent_id}/reset_client_services_file',
    RESET_AGENT_CLIENTS: '/for_agent/{agent_id}/reset_agent_clients_file',
    TOOL_RECORDS: '/for_agent/{agent_id}/tool_records',
    USE_TOOL: '/for_agent/{agent_id}/use_tool' // 向后兼容
  },

  // 监控和生命周期 API
  MONITORING: {
    AGENTS_SUMMARY: '/agents_summary',
    LIFECYCLE_CONFIG: '/lifecycle/config',
    HEALTH_SUMMARY: '/health/summary',
    HEALTH_SERVICE: '/health/service/{service_name}',
    HEALTH_CHECK: '/health/check/{service_name}',
    TOOLS_REFRESH: '/tools/refresh',
    TOOLS_REFRESH_SERVICE: '/tools/refresh/{service_name}',
    TOOLS_UPDATE_STATUS: '/tools/update_status',
    CONTENT_SNAPSHOT: '/content/snapshot/{service_name}',
    CONTENT_SNAPSHOTS: '/content/snapshots',
    LIFECYCLE_DISCONNECT: '/lifecycle/disconnect/{service_name}',
    ALERTS: '/monitoring/alerts',
    PERFORMANCE: '/monitoring/performance',
    USAGE_STATS: '/monitoring/usage_stats'
  },

  // 数据空间管理 API
  DATA_SPACE: {
    INFO: '/data_space/info',
    WORKSPACE_LIST: '/workspace/list',
    WORKSPACE_CREATE: '/workspace/create',
    WORKSPACE_SWITCH: '/workspace/switch',
    WORKSPACE_CURRENT: '/workspace/current',
    WORKSPACE_DELETE: '/workspace/{workspace_name}'
  },

  // LangChain 集成 API
  LANGCHAIN: {
    STORE_TOOLS: '/for_store/langchain_tools',
    STORE_SERVICE_TOOLS: '/for_store/langchain_tools/{service_name}',
    STORE_TOOL_EXECUTE: '/for_store/langchain_tool_execute',
    STORE_TOOL_INFO: '/for_store/langchain_tool_info/{tool_name}',
    AGENT_TOOLS: '/for_agent/{agent_id}/langchain_tools',
    AGENT_SERVICE_TOOLS: '/for_agent/{agent_id}/langchain_tools/{service_name}',
    AGENT_TOOL_EXECUTE: '/for_agent/{agent_id}/langchain_tool_execute'
  }
}

// 服务生命周期状态
export const SERVICE_LIFECYCLE_STATES = {
  INITIALIZING: 'initializing',
  HEALTHY: 'healthy',
  WARNING: 'warning',
  RECONNECTING: 'reconnecting',
  UNREACHABLE: 'unreachable',
  DISCONNECTING: 'disconnecting',
  DISCONNECTED: 'disconnected'
}

// API 响应状态码
export const API_STATUS_CODES = {
  SUCCESS: 200,
  CREATED: 201,
  BAD_REQUEST: 400,
  UNAUTHORIZED: 401,
  FORBIDDEN: 403,
  NOT_FOUND: 404,
  INTERNAL_ERROR: 500
}

// 错误类型
export const ERROR_TYPES = {
  INTERNAL_ERROR: 'INTERNAL_ERROR',
  VALIDATION_ERROR: 'VALIDATION_ERROR',
  NOT_FOUND: 'NOT_FOUND',
  UNAUTHORIZED: 'UNAUTHORIZED',
  FORBIDDEN: 'FORBIDDEN',
  SERVICE_NOT_FOUND: 'SERVICE_NOT_FOUND',
  SERVICE_OPERATION_FAILED: 'SERVICE_OPERATION_FAILED',
  AGENT_NOT_FOUND: 'AGENT_NOT_FOUND',
  TOOL_NOT_FOUND: 'TOOL_NOT_FOUND',
  CONFIG_ERROR: 'CONFIG_ERROR'
}

// 工具执行状态
export const TOOL_EXECUTION_STATUS = {
  PENDING: 'pending',
  RUNNING: 'running',
  SUCCESS: 'success',
  FAILED: 'failed',
  TIMEOUT: 'timeout'
}
