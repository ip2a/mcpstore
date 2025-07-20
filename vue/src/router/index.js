import { createRouter, createWebHistory } from 'vue-router'
import NProgress from 'nprogress'

// 路由组件懒加载
const Dashboard = () => import('@/views/Dashboard.vue')
const ServiceList = () => import('@/views/services/ServiceList.vue')
const ServiceAdd = () => import('@/views/services/ServiceAdd.vue')
const ServiceEdit = () => import('@/views/services/ServiceEdit.vue')
const LocalServices = () => import('@/views/services/LocalServices.vue')
const ToolList = () => import('@/views/tools/ToolList.vue')
const ToolExecute = () => import('@/views/tools/ToolExecute.vue')
const AgentList = () => import('@/views/agents/AgentList.vue')
const AgentCreate = () => import('@/views/agents/AgentCreate.vue')
const Monitoring = () => import('@/views/Monitoring.vue')
const Settings = () => import('@/views/Settings.vue')
const ResetManager = () => import('@/views/system/ResetManager.vue')

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
        path: 'local',
        name: 'LocalServices',
        component: LocalServices,
        meta: {
          title: '本地服务',
          icon: 'FolderOpened',
          keepAlive: true
        }
      }
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
        path: 'create',
        name: 'AgentCreate',
        component: AgentCreate,
        meta: {
          title: '创建Agent',
          icon: 'Plus'
        }
      }
    ]
  },
  {
    path: '/monitoring',
    name: 'Monitoring',
    component: Monitoring,
    meta: {
      title: '系统监控',
      icon: 'DataAnalysis',
      keepAlive: true
    }
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
      }
    ]
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
  history: createWebHistory(import.meta.env.BASE_URL),
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
  NProgress.start()
  
  // 设置页面标题
  if (to.meta.title) {
    document.title = `${to.meta.title} - MCPStore 管理面板`
  } else {
    document.title = 'MCPStore 管理面板'
  }
  
  next()
})

// 全局后置钩子
router.afterEach(() => {
  NProgress.done()
})

// 路由错误处理
router.onError((error) => {
  console.error('Router Error:', error)
  NProgress.done()
})

export default router
