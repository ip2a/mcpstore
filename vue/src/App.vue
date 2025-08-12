<template>
  <div id="app" class="app-container">
    <!-- 主布局 -->
    <el-container class="layout-container">
      <!-- 侧边栏 -->
      <el-aside 
        :width="isCollapse ? '64px' : '250px'" 
        class="sidebar"
        :class="{ 'sidebar-collapse': isCollapse }"
      >
        <div class="logo-container">
          <div class="logo">
            <el-icon v-if="isCollapse" size="24"><Setting /></el-icon>
            <template v-else>
              <el-icon size="24"><Setting /></el-icon>
              <span class="logo-text">MCPStore</span>
            </template>
          </div>
        </div>
        
        <el-menu
          :default-active="$route.path"
          :collapse="isCollapse"
          :unique-opened="false"
          router
          class="sidebar-menu"
        >
          <!-- 仪表板 -->
          <el-menu-item index="/dashboard">
            <el-icon><Monitor /></el-icon>
            <span>仪表板</span>
          </el-menu-item>

          <!-- 服务管理 -->
          <div class="menu-group" v-if="!isCollapse">
            <div class="menu-group-title">服务管理</div>
          </div>
          <el-menu-item index="/services/list">
            <el-icon><List /></el-icon>
            <span>服务列表</span>
          </el-menu-item>
          <el-menu-item index="/services/add">
            <el-icon><Plus /></el-icon>
            <span>添加服务</span>
          </el-menu-item>

          <!-- 工具管理 -->
          <div class="menu-group" v-if="!isCollapse">
            <div class="menu-group-title">工具管理</div>
          </div>
          <el-menu-item index="/tools/list">
            <el-icon><List /></el-icon>
            <span>工具列表</span>
          </el-menu-item>
          <el-menu-item index="/tools/execute">
            <el-icon><VideoPlay /></el-icon>
            <span>工具执行</span>
          </el-menu-item>

          <!-- Agent管理 -->
          <div class="menu-group" v-if="!isCollapse">
            <div class="menu-group-title">Agent管理</div>
          </div>
          <el-menu-item index="/agents/list">
            <el-icon><User /></el-icon>
            <span>Agent列表</span>
          </el-menu-item>

          <!-- 系统管理 -->
          <div class="menu-group" v-if="!isCollapse">
            <div class="menu-group-title">系统管理</div>
          </div>
          <el-menu-item index="/system/mcp-config">
            <el-icon><Document /></el-icon>
            <span>MCP配置</span>
          </el-menu-item>
          <el-menu-item index="/system/reset">
            <el-icon><RefreshLeft /></el-icon>
            <span>重置管理</span>
          </el-menu-item>
          <el-menu-item index="/settings">
            <el-icon><Setting /></el-icon>
            <span>系统设置</span>
          </el-menu-item>
        </el-menu>
      </el-aside>
      
      <!-- 主内容区 -->
      <el-container class="main-container">
        <!-- 顶部导航栏 -->
        <el-header class="header">
          <div class="header-left">
            <el-button 
              :icon="isCollapse ? Expand : Fold" 
              @click="toggleCollapse"
              text
              size="large"
            />
            <el-breadcrumb separator="/">
              <el-breadcrumb-item 
                v-for="item in breadcrumbs" 
                :key="item.path"
                :to="item.path"
              >
                {{ item.title }}
              </el-breadcrumb-item>
            </el-breadcrumb>
          </div>
          
          <div class="header-right">
            <!-- 外部链接 -->
            <div class="external-links">
              <!-- GitHub链接 -->
              <el-tooltip content="查看 GitHub 源码" placement="bottom">
                <el-button
                  text
                  circle
                  size="small"
                  @click="openGitHub"
                  class="link-button"
                >
                  <svg class="github-icon" viewBox="0 0 24 24" width="16" height="16">
                    <path fill="currentColor" d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
                  </svg>
                </el-button>
              </el-tooltip>

              <!-- PyPI链接 -->
              <el-tooltip content="安装 PyPI 包" placement="bottom">
                <el-button
                  text
                  circle
                  size="small"
                  @click="openPyPI"
                  class="link-button"
                >
                  <svg class="pypi-icon" viewBox="0 0 24 24" width="16" height="16">
                    <path fill="currentColor" d="M6.035 1.5h11.93c2.51 0 4.535 2.025 4.535 4.535v11.93c0 2.51-2.025 4.535-4.535 4.535H6.035C3.525 22.5 1.5 20.475 1.5 17.965V6.035C1.5 3.525 3.525 1.5 6.035 1.5zM12 6.75c-2.9 0-5.25 2.35-5.25 5.25s2.35 5.25 5.25 5.25 5.25-2.35 5.25-5.25S14.9 6.75 12 6.75zm0 2.25c1.655 0 3 1.345 3 3s-1.345 3-3 3-3-1.345-3-3 1.345-3 3-3z"/>
                  </svg>
                </el-button>
              </el-tooltip>
            </div>

            <!-- 系统状态 -->
            <el-badge
              :value="systemStatus.unhealthyServices"
              :hidden="systemStatus.unhealthyServices === 0"
              class="status-badge"
            >
              <el-button 
                :type="systemStatus.isHealthy ? 'success' : 'danger'"
                :icon="systemStatus.isHealthy ? SuccessFilled : WarningFilled"
                circle
                size="small"
                @click="checkSystemStatus"
              />
            </el-badge>
            
            <!-- 刷新按钮 -->
            <el-button 
              :icon="Refresh" 
              @click="refreshData"
              :loading="isRefreshing"
              circle
              size="small"
            />
            
            <!-- 主题切换 -->
            <el-switch
              v-model="isDark"
              :active-icon="Moon"
              :inactive-icon="Sunny"
              @change="toggleTheme"
            />
          </div>
        </el-header>
        
        <!-- 主内容 -->
        <el-main class="main-content">
          <router-view v-slot="{ Component }">
            <transition name="fade-transform" mode="out-in">
              <component :is="Component" />
            </transition>
          </router-view>
        </el-main>
      </el-container>
    </el-container>
    
    <!-- 全局加载遮罩已移除，保持静默体验 -->
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useAppStore } from '@/stores/app'
import { useSystemStore } from '@/stores/system'
import { useServicesStore } from '@/stores/services'
import { useToolsStore } from '@/stores/tools'
import { useToolExecutionStore } from '@/stores/toolExecution'
import {
  Setting, Monitor, List, Plus, FolderOpened,
  VideoPlay, User, Expand, Fold,
  SuccessFilled, WarningFilled, Refresh, Moon, Sunny, RefreshLeft, Document
} from '@element-plus/icons-vue'

const route = useRoute()
const appStore = useAppStore()
const systemStore = useSystemStore()
const servicesStore = useServicesStore()
const toolsStore = useToolsStore()
const toolExecutionStore = useToolExecutionStore()

// 响应式数据 - 使用store中的状态
const isCollapse = computed({
  get: () => appStore.isCollapse,
  set: (value) => appStore.setCollapse(value)
})

const isDark = computed({
  get: () => appStore.isDark,
  set: (value) => appStore.setTheme(value ? 'dark' : 'light')
})

const isRefreshing = computed(() => appStore.isLoading || systemStore.isLoading)

// 计算属性
const breadcrumbs = computed(() => {
  const matched = route.matched.filter(item => item.meta && item.meta.title)
  return matched.map(item => ({
    path: item.path,
    title: item.meta.title
  }))
})

const systemStatus = computed(() => systemStore.systemStatus)

// 方法
const toggleCollapse = () => {
  appStore.setCollapse(!appStore.isCollapse)
}

const toggleTheme = () => {
  const newTheme = appStore.isDark ? 'light' : 'dark'
  appStore.setTheme(newTheme)
  document.documentElement.classList.toggle('dark', newTheme === 'dark')
}

const refreshData = async () => {
  try {
    appStore.setGlobalLoading(true)

    // 并行刷新所有数据
    await Promise.all([
      systemStore.refreshAllData(),
      servicesStore.refreshAll(),
      toolsStore.fetchTools(true),
      toolExecutionStore.fetchToolRecords(50, true)
    ])

    appStore.addNotification({
      title: '数据刷新成功',
      message: '所有数据已更新',
      type: 'success'
    })
  } catch (error) {
    console.error('刷新数据失败:', error)
    appStore.addError({
      message: `数据刷新失败: ${error.message}`,
      type: 'refresh-error',
      source: 'App.vue'
    })
  } finally {
    appStore.setGlobalLoading(false)
  }
}

const checkSystemStatus = async () => {
  await systemStore.safeCheckSystemStatus()
}

// 打开外部链接
const openGitHub = () => {
  window.open('https://github.com/whillhill/mcpstore', '_blank')
}

const openPyPI = () => {
  window.open('https://pypi.org/project/mcpstore', '_blank')
}

// 生命周期
onMounted(async () => {
  try {
    // 初始化应用Store
    await appStore.initializeApp()

    // 初始化工具执行历史
    toolExecutionStore.loadHistoryFromStorage()

    // 获取系统状态（静默失败）
    await checkSystemStatus()

    console.log('🚀 App mounted and initialized')
  } catch (error) {
    console.error('App initialization failed:', error)
    appStore.addError({
      message: `应用初始化失败: ${error.message}`,
      type: 'initialization-error',
      source: 'App.vue'
    })
  }
})

// 监听主题变化
watch(() => appStore.currentTheme, (newTheme) => {
  document.documentElement.classList.toggle('dark', newTheme === 'dark')
  document.documentElement.setAttribute('data-theme', newTheme)
})

// 监听连接状态变化
watch(() => appStore.appState.connected, (connected) => {
  if (!connected) {
    console.warn('⚠️ Connection lost')
  } else {
    console.log('✅ Connection restored')
  }
})

// 监听错误状态
watch(() => appStore.hasErrors, (hasErrors) => {
  if (hasErrors && appStore.lastError) {
    const error = appStore.lastError
    if (error.type === 'critical') {
      ElMessage.error({
        message: error.message,
        duration: 0,
        showClose: true
      })
    }
  }
})

// 监听侧边栏状态变化
watch(isCollapse, (newVal) => {
  localStorage.setItem('mcpstore-collapse', newVal.toString())
})
</script>

<style lang="scss" scoped>
.app-container {
  height: 100vh;
  overflow: hidden;
}

.layout-container {
  height: 100%;
}

.sidebar {
  background: var(--el-bg-color);
  border-right: 1px solid var(--el-border-color);
  transition: width 0.3s ease;
  
  .logo-container {
    height: 60px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-bottom: 1px solid var(--el-border-color);
    
    .logo {
      display: flex;
      align-items: center;
      gap: 8px;
      font-size: 18px;
      font-weight: bold;
      color: var(--el-color-primary);
      
      .logo-text {
        transition: opacity 0.3s ease;
      }
    }
  }
  
  &.sidebar-collapse {
    .logo-text {
      opacity: 0;
    }
  }
  
  .sidebar-menu {
    border: none;
    height: calc(100vh - 60px);
    overflow-y: auto;

    .menu-group {
      padding: 12px 20px 8px;
      margin-top: 8px;

      .menu-group-title {
        font-size: 12px;
        color: var(--el-text-color-secondary);
        font-weight: 500;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        line-height: 1;
      }
    }

    .menu-group + .el-menu-item {
      margin-top: 4px;
    }

    .el-menu-item {
      margin: 2px 8px;
      border-radius: 6px;

      &:hover {
        background-color: var(--el-color-primary-light-9);
        color: var(--el-color-primary);
      }

      &.is-active {
        background-color: var(--el-color-primary-light-8);
        color: var(--el-color-primary);
        font-weight: 500;

        &::before {
          content: '';
          position: absolute;
          left: 0;
          top: 50%;
          transform: translateY(-50%);
          width: 3px;
          height: 20px;
          background-color: var(--el-color-primary);
          border-radius: 0 2px 2px 0;
        }
      }
    }
  }
}

.main-container {
  flex: 1;
  overflow: hidden;
}

.header {
  background: var(--el-bg-color);
  border-bottom: 1px solid var(--el-border-color);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 20px;
  
  .header-left {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  
  .header-right {
    display: flex;
    align-items: center;
    gap: 12px;

    .external-links {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-right: 8px;
      padding-right: 8px;
      border-right: 1px solid var(--el-border-color-light);
    }

    .link-button {
      color: var(--el-text-color-regular) !important;
      transition: all 0.3s ease;

      &:hover {
        color: var(--el-color-primary) !important;
        transform: scale(1.1);
      }
    }

    .github-icon,
    .pypi-icon {
      width: 16px;
      height: 16px;
      transition: all 0.3s ease;
    }

    .link-button:hover .github-icon {
      color: #333;
    }

    .link-button:hover .pypi-icon {
      color: #3775A9;
    }

    .status-badge {
      margin-right: 8px;
    }
  }
}

.main-content {
  padding: 20px;
  overflow-y: auto;
  background: var(--el-bg-color-page);
}

// 过渡动画
.fade-transform-enter-active,
.fade-transform-leave-active {
  transition: all 0.3s ease;
}

.fade-transform-enter-from {
  opacity: 0;
  transform: translateX(30px);
}

.fade-transform-leave-to {
  opacity: 0;
  transform: translateX(-30px);
}

// 暗色模式适配
.dark {
  .external-links {
    border-right-color: var(--el-border-color-darker);
  }

  .link-button:hover .github-icon {
    color: #fff;
  }

  .link-button:hover .pypi-icon {
    color: #4A90E2;
  }
}

// 响应式设计
@media (max-width: 768px) {
  .external-links {
    gap: 4px !important;
    margin-right: 4px !important;
    padding-right: 4px !important;
  }

  .header-right {
    gap: 8px !important;
  }
}
</style>
