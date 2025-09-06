import { createRouter, createWebHistory } from 'vue-router'

// 路由组件懒加载
const Dashboard = () => import('@/views/DashboardAdvanced.vue')
const ServiceList = () => import('@/views/services/ServiceList.vue')
const ServiceAdd = () => import('@/views/services/ServiceAdd.vue')
const ServiceEdit = () => import('@/views/services/ServiceEdit.vue')
const ServiceDetail = () => import('@/views/services/ServiceDetail.vue')

const ToolList = () => import('@/views/tools/ToolList.vue')
const ToolExecute = () => import('@/views/tools/ToolExecute.vue')
const AgentList = () => import('@/views/agents/AgentList.vue')
const AgentDetail = () => import('@/views/agents/AgentDetail.vue')
const AgentServiceAdd = () => import('@/views/agents/ServiceAdd.vue')

const Settings = () => import('@/views/Settings.vue')
const ResetManager = () => import('@/views/system/ResetManager.vue')
const TestPage = () => import('@/views/TestPage.vue')
const DashboardSimple = () => import('@/views/DashboardSimple.vue')
const ApiDebugPage = () => import('@/views/ApiDebugPage.vue')
const McpConfigManager = () => import('@/views/config/McpConfigManager.vue')
const ConfigEditor = () => import('@/views/config/ConfigEditor.vue')
const WorkspaceManager = () => import('@/views/WorkspaceManager.vue')
const ServiceAnalytics = () => import('@/views/analytics/ServiceAnalytics.vue')
const ToolTemplates = () => import('@/views/templates/ToolTemplates.vue')
const AdvancedSearch = () => import('@/views/search/AdvancedSearch.vue')

const routes = [
  {
    path: '/',
    redirect: '/dashboard'
  },
  {
    path: '/dashboard',
    name: 'Dashboard',
    component: Dashboard,
    meta: {
      title: '仪表板',
      icon: 'Monitor',
      keepAlive: true
    }
  },
  {
    path: '/services',
    name: 'Services',
    meta: {
      title: '服务管理',
      icon: 'Connection'
    },
    children: [
      {
        path: 'list',
        name: 'ServiceList',
        component: ServiceList,
        meta: {
          title: '服务列表',
          icon: 'List',
          keepAlive: true
        }
      },
      {
        path: 'add',
        name: 'ServiceAdd',
        component: ServiceAdd,
        meta: {
          title: '添加服务',
          icon: 'Plus'
        }
      },
      {
        path: 'edit/:serviceName',
        name: 'ServiceEdit',
        component: ServiceEdit,
        meta: {
          title: '编辑服务',
          icon: 'Edit'
        }
      },
      {
        path: 'detail/:serviceName',
        name: 'ServiceDetail',
        component: ServiceDetail,
        meta: {
          title: '服务详情',
          icon: 'View'
        }
      },

    ]
  },
  {
    path: '/tools',
    name: 'Tools',
    meta: {
      title: '工具管理',
      icon: 'Tools'
    },
    children: [
      {
        path: 'list',
        name: 'ToolList',
        component: ToolList,
        meta: {
          title: '工具列表',
          icon: 'List',
          keepAlive: true
        }
      },
      {
        path: 'execute',
        name: 'ToolExecute',
        component: ToolExecute,
        meta: {
          title: '工具执行',
          icon: 'VideoPlay'
        }
      }
    ]
  },
  {
    path: '/analytics',
    name: 'ServiceAnalytics',
    component: ServiceAnalytics,
    meta: {
      title: '服务分析',
      icon: 'TrendCharts',
      keepAlive: true
    }
  },
  {
    path: '/templates',
    name: 'ToolTemplates',
    component: ToolTemplates,
    meta: {
      title: '工具模板',
      icon: 'Document',
      keepAlive: true
    }
  },
  {
    path: '/agents',
    name: 'Agents',
    meta: {
      title: 'Agent管理',
      icon: 'User'
    },
    children: [
      {
        path: 'list',
        name: 'AgentList',
        component: AgentList,
        meta: {
          title: 'Agent列表',
          icon: 'List',
          keepAlive: true
        }
      },
      {
        path: ':id/detail',
        name: 'AgentDetail',
        component: AgentDetail,
        meta: {
          title: 'Agent详情',
          icon: 'View'
        }
      },
      {
        path: 'service-add',
        name: 'AgentServiceAdd',
        component: AgentServiceAdd,
        meta: {
          title: '添加服务',
          icon: 'Plus'
        }
      }
    ]
  },
  {
    path: '/settings',
    name: 'Settings',
    component: Settings,
    meta: {
      title: '系统设置',
      icon: 'Setting'
    }
  },
  {
    path: '/system',
    name: 'System',
    meta: {
      title: '系统管理',
      icon: 'Tools'
    },
    children: [
      {
        path: 'reset',
        name: 'ResetManager',
        component: ResetManager,
        meta: {
          title: '重置管理',
          icon: 'RefreshLeft',
          keepAlive: false
        }
      },
      {
        path: '/system/test',
        name: 'TestPage',
        component: TestPage,
        meta: {
          title: '环境测试',
          icon: 'Monitor',
          keepAlive: false
        }
      },
      {
        path: '/system/dashboard-simple',
        name: 'DashboardSimple',
        component: DashboardSimple,
        meta: {
          title: '简化仪表板',
          icon: 'DataBoard',
          keepAlive: false
        }
      },
      {
        path: '/system/api-debug',
        name: 'ApiDebugPage',
        component: ApiDebugPage,
        meta: {
          title: 'API调试',
          icon: 'Tools',
          keepAlive: false
        }
      },
      {
        path: '/system/mcp-config',
        name: 'McpConfigManager',
        component: McpConfigManager,
        meta: {
          title: 'MCP配置管理',
          icon: 'Document',
          keepAlive: false
        }
      },
      {
        path: '/system/config-editor',
        name: 'ConfigEditor',
        component: ConfigEditor,
        meta: {
          title: '高级配置编辑器',
          icon: 'Edit',
          keepAlive: false
        }
      },
      {
        path: '/system/workspace',
        name: 'WorkspaceManager',
        component: WorkspaceManager,
        meta: {
          title: '工作空间管理',
          icon: 'FolderOpened',
          keepAlive: false
        }
      }
    ]
  },
  {
    path: '/search',
    name: 'AdvancedSearch',
    component: AdvancedSearch,
    meta: {
      title: '高级搜索',
      icon: 'Search',
      keepAlive: true
    }
  },
  {
    path: '/:pathMatch(.*)*',
    name: 'NotFound',
    component: () => import('@/views/NotFound.vue'),
    meta: {
      title: '页面未找到'
    }
  }
]

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL || '/'),
  routes,
  scrollBehavior(to, from, savedPosition) {
    if (savedPosition) {
      return savedPosition
    } else {
      return { top: 0 }
    }
  }
})

// 全局前置守卫
router.beforeEach((to, from, next) => {
  // 不启动NProgress，保持静默导航

  // 设置页面标题
  if (to.meta.title) {
    document.title = `${to.meta.title} - MCPStore 管理面板`
  } else {
    document.title = 'MCPStore 管理面板'
  }

  next()
})

// 全局后置钩子
router.afterEach((to, from) => {
  // 静默导航，不使用NProgress
})

// 路由错误处理
router.onError((error) => {
  console.error('Router Error:', error)
})

export default router
