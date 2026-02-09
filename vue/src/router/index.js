import { createRouter, createWebHistory } from 'vue-router'

// 路由组件懒加载
const Dashboard = () => import('@/views/Dashboard.vue')
const ServiceList = () => import('@/views/services/ServiceList.vue')
const ServiceAdd = () => import('@/views/services/ServiceAdd.vue')
const ServiceEdit = () => import('@/views/services/ServiceEdit.vue')
const ServiceDetail = () => import('@/views/services/ServiceDetail.vue')

const ToolList = () => import('@/views/tools/ToolList.vue')
const ToolExecute = () => import('@/views/tools/ToolExecute.vue')
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
    redirect: '/system/dashboard'
  },
  {
    path: '/external/github',
    name: 'external_github',
    component: ExternalLink,
    meta: {
      title: 'GitHub Project',
      icon: 'Link',
      keepAlive: false,
      url: import.meta.env.VITE_GITHUB_URL || 'https://github.com/whillhill/mcpstore',
      description: 'Visit the MCPStore project repository on GitHub to view source code, report issues, and contribute.'
    }
  },
  {
    path: '/external/pypi',
    name: 'external_pypi',
    component: ExternalLink,
    meta: {
      title: 'PyPI Package',
      icon: 'Link',
      keepAlive: false,
      url: import.meta.env.VITE_PYPI_URL || 'https://pypi.org/project/mcpstore',
      description: 'View the MCPStore package on PyPI for installation instructions and version history.'
    }
  },
  {
    path: '/docs',
    name: 'system_docs_index',
    component: ExternalEmbed,
    meta: {
      title: '文档中心',
      icon: 'Reading',
      keepAlive: true,
      url: import.meta.env.VITE_DOCS_URL || 'https://doc.mcpstore.wiki/'
    }
  },
  // system
  {
    path: '/system/dashboard',
    name: 'system_dashboard',
    component: Dashboard,
    meta: {
      title: '仪表板',
      icon: 'Monitor',
      keepAlive: true
    }
  },
  // for_store - services
  {
    path: '/for_store/list_services',
    name: 'for_store_list_services',
    component: ServiceList,
    meta: { title: '服务列表', icon: 'Connection', keepAlive: true }
  },
  {
    path: '/for_store/add_service',
    name: 'for_store_add_service',
    component: ServiceAdd,
    meta: { title: '添加服务', icon: 'Plus', hidden: true }
  },
  {
    path: '/for_store/update_service/:serviceName',
    name: 'for_store_update_service',
    component: ServiceEdit,
    meta: { title: '编辑服务', icon: 'Edit', hidden: true }
  },
  {
    path: '/for_store/service_info/:serviceName',
    name: 'for_store_service_info',
    component: ServiceDetail,
    meta: { title: '服务详情', icon: 'View', hidden: true }
  },
  // for_store - tools
  {
    path: '/for_store/list_tools',
    name: 'for_store_list_tools',
    component: ToolList,
    meta: { title: '工具列表', icon: 'Tools', keepAlive: true }
  },
  {
    path: '/for_store/call_tool',
    name: 'for_store_call_tool',
    component: ToolExecute,
    meta: { title: '工具执行', icon: 'VideoPlay', hidden: true }
  },
  // for_store - agents
  {
    path: '/for_store/list_agents',
    name: 'for_store_list_agents',
    component: AgentList,
    meta: { title: 'Agent列表', icon: 'User', keepAlive: true }
  },
  {
    path: '/for_store/agent_detail/:id',
    name: 'for_store_agent_detail',
    component: AgentDetail,
    meta: { title: 'Agent详情', icon: 'UserFilled', hidden: true }
  },
  // for_agent - add_service (保留功能入口)
  {
    path: '/for_agent/:agent_id/add_service',
    name: 'for_agent_add_service',
    component: AgentServiceAdd,
    meta: { title: '为Agent添加服务', icon: 'Plus', hidden: true }
  },
  // for_store - config/cache
  {
    path: '/for_store/show_config',
    name: 'for_store_show_config',
    component: ConfigCenter,
    meta: { title: '配置中心', icon: 'Setting', keepAlive: true }
  },
  {
    path: '/for_store/show_cache',
    name: 'for_store_show_cache',
    component: CacheSpace,
    meta: { title: '缓存空间', icon: 'Coin', keepAlive: true }
  },
  // Redirects from old paths
  { path: '/dashboard', redirect: '/system/dashboard', meta: { hidden: true } },
  { path: '/services', redirect: '/for_store/list_services', meta: { hidden: true } },
  { path: '/services/add', redirect: '/for_store/add_service', meta: { hidden: true } },
  { path: '/services/edit/:serviceName', redirect: '/for_store/update_service/:serviceName', meta: { hidden: true } },
  { path: '/services/detail/:serviceName', redirect: '/for_store/service_info/:serviceName', meta: { hidden: true } },
  { path: '/tools', redirect: '/for_store/list_tools', meta: { hidden: true } },
  { path: '/tools/execute', redirect: '/for_store/call_tool', meta: { hidden: true } },
  { path: '/agents', redirect: '/for_store/list_agents', meta: { hidden: true } },
  { path: '/agents/:id/detail', redirect: '/for_store/list_agents', meta: { hidden: true } },
  { path: '/agents/service-add', redirect: '/for_store/add_service', meta: { hidden: true } },
  { path: '/config', redirect: '/for_store/show_config', meta: { hidden: true } },
  { path: '/cache', redirect: '/for_store/show_cache', meta: { hidden: true } },
  {
    path: '/:pathMatch(.*)*',
    name: 'system_not_found',
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
  const appTitle = import.meta.env.VITE_APP_TITLE || 'MCPStore 管理面板'
  if (to.meta.title) {
    document.title = `${to.meta.title} - ${appTitle}`
  } else {
    document.title = appTitle
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
