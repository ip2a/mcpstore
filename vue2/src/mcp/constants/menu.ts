/**
 * MCP菜单配置
 * 统一管理所有MCP相关的菜单配置和外链
 */

import { AppRouteRecord } from '@/types/router'

/**
 * MCP外链配置
 */
export const MCP_EXTERNAL_LINKS = {
  OFFICIAL_DOCS: 'https://doc.mcpstore.wiki/',
  GITHUB_REPO: 'https://github.com/whillhill/mcpstore',
  PYPI_PACKAGE: 'https://pypi.org/project/mcpstore',
  README: 'https://github.com/whillhill/mcpstore'
} as const

/**
 * MCP路由别名
 * 集中管理MCP相关页面的路由路径
 */
export const MCP_ROUTES = {
  // 主要页面
  DASHBOARD: '/mcp/views/dashboard/index',
  SERVICE_LIST: '/mcp/views/services/index',
  ADD_SERVICE: '/mcp/views/services/add',
  TOOL_LIST: '/mcp/views/tools/index',
  TOOL_EXECUTE: '/mcp/views/tools/execute',
  CONFIG_MANAGER: '/mcp/views/config/index',
  AGENTS_LIST: '/mcp/views/agents/index'
} as const

/**
 * MCP菜单配置
 * 包含所有MCP相关的页面和外链菜单
 */
export const MCP_MENU_CONFIG: AppRouteRecord[] = [
  // MCP 仪表盘
  {
    name: 'McpDashboard',
    path: '/dashboard',
    component: MCP_ROUTES.DASHBOARD,
    meta: {
      title: 'MCP 仪表盘',
      icon: '&#xe721;',
      keepAlive: false,
      fixedTab: true,
      roles: ['R_SUPER', 'R_ADMIN']
    }
  },
  // 服务管理
  {
    name: 'ServiceList',
    path: '/services',
    component: MCP_ROUTES.SERVICE_LIST,
    meta: {
      title: '服务列表',
      icon: '&#xe7aa;',
      keepAlive: false,
      roles: ['R_SUPER', 'R_ADMIN']
    }
  },
  {
    name: 'AddService',
    path: '/add-service',
    component: MCP_ROUTES.ADD_SERVICE,
    meta: {
      title: '添加服务',
      icon: '&#xe812;',
      keepAlive: false,
      roles: ['R_SUPER', 'R_ADMIN']
    }
  },
  // 工具管理
  {
    name: 'ToolList',
    path: '/tools',
    component: MCP_ROUTES.TOOL_LIST,
    meta: {
      title: '工具列表',
      icon: '&#xe82a;',
      keepAlive: false,
      roles: ['R_SUPER', 'R_ADMIN']
    }
  },
  {
    name: 'ToolExecute',
    path: '/tools/execute',
    component: MCP_ROUTES.TOOL_EXECUTE,
    meta: {
      title: '工具执行器',
      icon: '&#xe828;',
      keepAlive: false,
      roles: ['R_SUPER', 'R_ADMIN']
    }
  },
  // 系统管理
  {
    name: 'ConfigManager',
    path: '/config-manager',
    component: MCP_ROUTES.CONFIG_MANAGER,
    meta: {
      title: '配置管理',
      icon: '&#xe815;',
      keepAlive: false,
      roles: ['R_SUPER', 'R_ADMIN']
    }
  },
  {
    name: 'AgentsList',
    path: '/agents',
    component: MCP_ROUTES.AGENTS_LIST,
    meta: {
      title: 'Agent管理',
      icon: '&#xe82a;',
      keepAlive: false,
      roles: ['R_SUPER', 'R_ADMIN']
    }
  },
  // 外链菜单
  {
    name: 'OfficialDocs',
    path: '',
    component: '',
    meta: {
      title: '官方文档',
      icon: '&#xe73e;',
      link: MCP_EXTERNAL_LINKS.OFFICIAL_DOCS,
      isIframe: false,
      keepAlive: false
    }
  },
  {
    name: 'Readme',
    path: '',
    component: '',
    meta: {
      title: 'README',
      icon: '&#xe7ae;',
      link: MCP_EXTERNAL_LINKS.README,
      isIframe: true,
      keepAlive: false
    }
  },
  {
    name: 'GitHub',
    path: '',
    component: '',
    meta: {
      title: 'GitHub仓库',
      icon: '&#xe6f6;',
      link: MCP_EXTERNAL_LINKS.GITHUB_REPO,
      isIframe: false,
      keepAlive: false
    }
  },
  {
    name: 'PyPI',
    path: '',
    component: '',
    meta: {
      title: 'PyPI仓库',
      icon: '&#xe6f7;',
      link: MCP_EXTERNAL_LINKS.PYPI_PACKAGE,
      isIframe: false,
      keepAlive: false
    }
  }
]

/**
 * 获取MCP菜单配置
 */
export function getMcpMenuConfig(): AppRouteRecord[] {
  return MCP_MENU_CONFIG
}

/**
 * 根据角色过滤MCP菜单
 */
export function filterMcpMenuByRoles(roles: string[]): AppRouteRecord[] {
  return MCP_MENU_CONFIG.filter(item => {
    const itemRoles = item.meta?.roles
    return !itemRoles || itemRoles.some(role => roles.includes(role))
  })
}
