import { createRouter, createWebHistory } from 'vue-router'

// 路由组件懒加载
const Dashboard = () => import('@/views/DashboardAdvanced.vue')
const ServiceList = () => import('@/views/services/ServiceList.vue')
const ServiceAdd = () => import('@/views/services/ServiceAdd.vue')
const ServiceEdit = () => import('@/views/services/ServiceEdit.vue')
const ServiceDetail = () => import('@/views/services/ServiceDetail.vue')

const ToolList = () => import('@/views/tools/ToolList.vue')
const ToolExecute = () => import('@/views/tools/ToolExecute.vue')
const ToolRecords = () => import('@/views/tools/ToolRecords.vue')
const AgentList = () => import('@/views/agents/AgentList.vue')
const AgentDetail = () => import('@/views/agents/AgentDetail.vue')
const AgentServiceAdd = () => import('@/views/agents/ServiceAdd.vue')

const ConfigCenter = () => import('@/views/config/ConfigCenter.vue')
const CacheSpace = () => import('@/views/CacheSpace.vue')
const ExternalEmbed = () => import('@/views/ExternalEmbed.vue')
const ExternalLink = () => import('@/views/ExternalLink.vue')

const routes = [
  {
    path: '/',
    redirect: '/dashboard'
  },
  {
    path: '/external/github',
    name: 'ExternalGitHub',
    component: ExternalLink,
    meta: {
      title: 'GitHub Project',
      icon: 'Link',
      keepAlive: false,
      url: 'https://github.com/whillhill/mcpstore',
      description: 'Visit the MCPStore project repository on GitHub to view source code, report issues, and contribute.'
    }
  },
  {
    path: '/external/pypi',
    name: 'ExternalPyPI',
    component: ExternalLink,
    meta: {
      title: 'PyPI Package',
      icon: 'Link',
      keepAlive: false,
      url: 'https://pypi.org/project/mcpstore',
      description: 'View the MCPStore package on PyPI for installation instructions and version history.'
    }
  },
  {
    path: '/docs',
    name: 'Documentation',
    component: ExternalEmbed,
    meta: {
      title: '文档中心',
      icon: 'Reading',
      keepAlive: true,
      url: 'https://doc.mcpstore.wiki/'
    }
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
    name: 'ServiceList',
    component: ServiceList,
    meta: {
      title: '服务列表',
      icon: 'Connection',
      keepAlive: true
    }
  },
  {
    path: '/services/add',
    name: 'ServiceAdd',
    component: ServiceAdd,
    meta: {
      title: '添加服务',
      icon: 'Plus',
      hidden: true
    }
  },
  {
    path: '/services/edit/:serviceName',
    name: 'ServiceEdit',
    component: ServiceEdit,
    meta: {
      title: '编辑服务',
      icon: 'Edit',
      hidden: true
    }
  },
  {
    path: '/services/detail/:serviceName',
    name: 'ServiceDetail',
    component: ServiceDetail,
    meta: {
      title: '服务详情',
      icon: 'View',
      hidden: true
    }
  },
  {
    path: '/tools',
    name: 'ToolList',
    component: ToolList,
    meta: {
      title: '工具列表',
      icon: 'Tools',
      keepAlive: true
    }
  },
  {
    path: '/tools/execute',
    name: 'ToolExecute',
    component: ToolExecute,
    meta: {
      title: '工具执行',
      icon: 'VideoPlay',
      hidden: true
    }
  },
  {
    path: '/tools/records',
    name: 'ToolRecords',
    component: ToolRecords,
    meta: {
      title: '工具记录',
      icon: 'Document',
      keepAlive: true
    }
  },
  {
    path: '/agents',
    name: 'AgentList',
    component: AgentList,
    meta: {
      title: 'Agent列表',
      icon: 'User',
      keepAlive: true
    }
  },
  {
    path: '/agents/:id/detail',
    name: 'AgentDetail',
    component: AgentDetail,
    meta: {
      title: 'Agent详情',
      icon: 'View',
      hidden: true
    }
  },
  {
    path: '/agents/service-add',
    name: 'AgentServiceAdd',
    component: AgentServiceAdd,
    meta: {
      title: '添加服务',
      icon: 'Plus',
      hidden: true
    }
  },
  {
    path: '/config',
    name: 'ConfigCenter',
    component: ConfigCenter,
    meta: {
      title: '配置中心',
      icon: 'Setting',
      keepAlive: true
    }
  },
  {
    path: '/cache',
    name: 'CacheSpace',
    component: CacheSpace,
    meta: {
      title: '缓存空间',
      icon: 'Coin',
      keepAlive: true
    }
  },
  {
    path: '/:pathMatch(.*)*',
    name: 'NotFound',
    component: () => import('@/views/NotFound.vue'),
    meta: {
      title: '页面未找到',
      hidden: true
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
