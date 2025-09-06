<template>
  <div id="app" class="app-container">
    <!-- 主布局 -->
    <el-container class="layout-container">
      <!-- 侧边栏 -->
      <el-aside 
        :width="isCollapse ? '64px' : '260px'" 
        class="sidebar"
        :class="{ 'sidebar-collapse': isCollapse, 'sidebar-dark': isDarkTheme }"
      >
        <div class="logo-container">
          <div class="logo">
            <el-icon v-if="isCollapse" size="24"><Platform /></el-icon>
            <template v-else>
              <el-icon size="24"><Platform /></el-icon>
              <span class="logo-text">MCPStore</span>
              <span class="logo-version">v1.0</span>
            </template>
          </div>
        </div>
        
        <el-scrollbar class="sidebar-scrollbar">
          <el-menu
            :default-active="$route.path"
            :collapse="isCollapse"
            :unique-opened="true"
            router
            class="sidebar-menu"
            :class="{ 'menu-dark': isDarkTheme }"
          >
            <!-- 仪表板 -->
            <el-menu-item index="/dashboard" class="menu-item-dashboard">
              <el-icon><Monitor /></el-icon>
              <span>仪表板</span>
            </el-menu-item>

            <!-- 高级搜索 -->
            <el-menu-item index="/search">
              <el-icon><Search /></el-icon>
              <span>高级搜索</span>
            </el-menu-item>

            <!-- 服务管理 -->
            <el-sub-menu index="/services" v-if="!isCollapse">
              <template #title>
                <el-icon><Connection /></el-icon>
                <span>服务管理</span>
              </template>
              <el-menu-item index="/services/list">
                <el-icon><List /></el-icon>
                <span>服务列表</span>
              </el-menu-item>
              <el-menu-item index="/services/add">
                <el-icon><Plus /></el-icon>
                <span>添加服务</span>
              </el-menu-item>
            </el-sub-menu>

            <!-- 工具管理 -->
            <el-sub-menu index="/tools" v-if="!isCollapse">
              <template #title>
                <el-icon><Tools /></el-icon>
                <span>工具管理</span>
              </template>
              <el-menu-item index="/tools/list">
                <el-icon><List /></el-icon>
                <span>工具列表</span>
              </el-menu-item>
              <el-menu-item index="/tools/execute">
                <el-icon><VideoPlay /></el-icon>
                <span>工具执行</span>
              </el-menu-item>
            </el-sub-menu>

            <!-- 工具模板 -->
            <el-menu-item index="/templates">
              <el-icon><Document /></el-icon>
              <span>工具模板</span>
            </el-menu-item>

            <!-- Agent管理 -->
            <el-sub-menu index="/agents" v-if="!isCollapse">
              <template #title>
                <el-icon><User /></el-icon>
                <span>Agent管理</span>
              </template>
              <el-menu-item index="/agents/list">
                <el-icon><List /></el-icon>
                <span>Agent列表</span>
              </el-menu-item>
              <el-menu-item index="/agents/service-add">
                <el-icon><Plus /></el-icon>
                <span>添加服务</span>
              </el-menu-item>
            </el-sub-menu>

            <!-- 数据分析 -->
            <el-menu-item index="/analytics">
              <el-icon><TrendCharts /></el-icon>
              <span>服务分析</span>
            </el-menu-item>

            <!-- 系统管理 -->
            <el-sub-menu index="/system" v-if="!isCollapse">
              <template #title>
                <el-icon><Setting /></el-icon>
                <span>系统管理</span>
              </template>
              <el-menu-item index="/system/mcp-config">
                <el-icon><Document /></el-icon>
                <span>MCP配置</span>
              </el-menu-item>
              <el-menu-item index="/system/config-editor">
                <el-icon><Edit /></el-icon>
                <span>高级配置编辑器</span>
              </el-menu-item>
              <el-menu-item index="/system/workspace">
                <el-icon><FolderOpened /></el-icon>
                <span>工作空间管理</span>
              </el-menu-item>
              <el-menu-item index="/system/reset">
                <el-icon><RefreshLeft /></el-icon>
                <span>重置管理</span>
              </el-menu-item>
            </el-sub-menu>

            <!-- 系统设置 -->
            <el-menu-item index="/settings">
              <el-icon><Setting /></el-icon>
              <span>系统设置</span>
            </el-menu-item>
          </el-menu>
        </el-scrollbar>
      </el-aside>
      
      <!-- 主内容区 -->
      <el-container class="main-container">
        <!-- 顶部导航栏 -->
        <el-header class="header" :class="{ 'header-dark': isDarkTheme }">
          <div class="header-left">
            <el-button 
              :icon="isCollapse ? Expand : Fold" 
              @click="toggleCollapse"
              text
              size="large"
              class="collapse-btn"
            />
            <el-breadcrumb separator="/" class="breadcrumb">
              <el-breadcrumb-item 
                v-for="item in breadcrumbs" 
                :key="item.path"
                :to="item.path"
                class="breadcrumb-item"
              >
                {{ item.title }}
              </el-breadcrumb-item>
            </el-breadcrumb>
          </div>
          
          <div class="header-right">
            <!-- 外部链接 -->
            <div class="external-links">
              <el-tooltip content="GitHub" placement="bottom">
                <el-button
                  text
                  circle
                  size="small"
                  @click="openGitHub"
                  class="link-button github-btn"
                >
                  <svg class="github-icon" viewBox="0 0 24 24" width="16" height="16">
                    <path fill="currentColor" d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
                  </svg>
                </el-button>
              </el-tooltip>

              <el-tooltip content="PyPI" placement="bottom">
                <el-button
                  text
                  circle
                  size="small"
                  @click="openPyPI"
                  class="link-button pypi-btn"
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
                class="status-btn"
              />
            </el-badge>
            
            <!-- 刷新按钮 -->
            <el-button 
              :icon="Refresh" 
              @click="refreshData"
              :loading="isRefreshing"
              circle
              size="small"
              class="refresh-btn"
            />
            
            <!-- 主题切换 -->
            <el-switch
              v-model="isDark"
              :active-icon="Moon"
              :inactive-icon="Sunny"
              @change="toggleTheme"
              class="theme-switch"
            />
          </div>
        </el-header>
        
        <!-- 主内容 -->
        <el-main class="main-content" :class="{ 'main-content-dark': isDarkTheme }">
          <router-view v-slot="{ Component }">
            <transition name="fade-transform" mode="out-in">
              <component :is="Component" />
            </transition>
          </router-view>
        </el-main>
      </el-container>
    </el-container>
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
  Platform, Setting, Monitor, Search, List, Plus, FolderOpened,
  VideoPlay, User, Expand, Fold, Connection, Tools,
  SuccessFilled, WarningFilled, Refresh, Moon, Sunny, RefreshLeft, Document, TrendCharts
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

const isDarkTheme = computed(() => appStore.currentTheme === 'dark')

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
  background: var(--bg-color-page);
}

.layout-container {
  height: 100%;
}

// 侧边栏样式
.sidebar {
  background: var(--bg-color);
  border-right: 1px solid var(--border-lighter);
  transition: width var(--transition-normal);
  box-shadow: var(--shadow-sm);
  position: relative;
  z-index: 1000;
  
  &.sidebar-dark {
    background: var(--bg-color);
    border-right-color: var(--border-light);
  }
  
  .logo-container {
    height: 64px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-bottom: 1px solid var(--border-lighter);
    background: linear-gradient(135deg, var(--primary-color) 0%, var(--primary-dark) 100%);
    overflow: hidden;
    position: relative;
    
    &::before {
      content: '';
      position: absolute;
      top: 0;
      left: 0;
      right: 0;
      bottom: 0;
      background: linear-gradient(45deg, transparent 30%, rgba(255,255,255,0.1) 50%, transparent 70%);
      animation: shimmer 3s infinite;
    }
    
    .logo {
      display: flex;
      align-items: center;
      gap: 12px;
      font-size: var(--font-size-xl);
      font-weight: var(--font-weight-bold);
      color: var(--text-inverse);
      position: relative;
      z-index: 1;
      
      .logo-text {
        transition: opacity var(--transition-normal);
        font-family: var(--font-family-sans);
        letter-spacing: -0.5px;
      }
      
      .logo-version {
        font-size: var(--font-size-xs);
        opacity: 0.8;
        font-weight: var(--font-weight-normal);
        background: rgba(255,255,255,0.2);
        padding: 2px 6px;
        border-radius: var(--border-radius-sm);
        margin-left: 4px;
      }
    }
  }
  
  &.sidebar-collapse {
    .logo-text,
    .logo-version {
      opacity: 0;
      width: 0;
      overflow: hidden;
    }
  }
  
  .sidebar-scrollbar {
    height: calc(100vh - 64px);
    
    .el-scrollbar__view {
      padding: 16px 8px;
    }
  }
  
  .sidebar-menu {
    border: none;
    
    &.menu-dark {
      .el-menu-item,
      .el-sub-menu__title {
        &:hover {
          background-color: var(--primary-lighter);
        }
        
        &.is-active {
          background-color: var(--primary-lighter);
          color: var(--primary-color);
        }
      }
    }
    
    .el-menu-item,
    .el-sub-menu__title {
      margin: 2px 8px;
      border-radius: var(--border-radius-md);
      height: 48px;
      line-height: 48px;
      transition: var(--transition-fast);
      
      &:hover {
        background-color: var(--primary-lighter);
        color: var(--primary-color);
        transform: translateX(2px);
      }
      
      &.is-active {
        background-color: var(--primary-lighter);
        color: var(--primary-color);
        font-weight: var(--font-weight-semibold);
        position: relative;
        
        &::before {
          content: '';
          position: absolute;
          left: 8px;
          top: 50%;
          transform: translateY(-50%);
          width: 4px;
          height: 24px;
          background-color: var(--primary-color);
          border-radius: var(--border-radius-sm);
        }
        
        .el-icon {
          color: var(--primary-color);
        }
      }
      
      .el-icon {
        font-size: 18px;
        color: var(--text-secondary);
        transition: var(--transition-fast);
        margin-right: 12px;
      }
      
      span {
        font-size: var(--font-size-sm);
      }
    }
    
    .el-sub-menu {
      .el-menu-item {
        margin-left: 8px;
        padding-left: 48px !important;
        
        &::before {
          left: 12px;
        }
      }
    }
    
    .menu-item-dashboard {
      margin-bottom: 8px;
      
      &.is-active {
        background: linear-gradient(135deg, var(--primary-color) 0%, var(--primary-dark) 100%);
        color: var(--text-inverse);
        
        &::before {
          background-color: var(--text-inverse);
        }
        
        .el-icon {
          color: var(--text-inverse);
        }
        
        &:hover {
          background: linear-gradient(135deg, var(--primary-color) 0%, var(--primary-dark) 100%);
        }
      }
    }
  }
}

// 主容器样式
.main-container {
  flex: 1;
  overflow: hidden;
  background: var(--bg-color-page);
}

// 顶部导航栏样式
.header {
  background: var(--bg-color);
  border-bottom: 1px solid var(--border-lighter);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 24px;
  height: 64px;
  box-shadow: var(--shadow-xs);
  transition: var(--transition-normal);
  
  &.header-dark {
    background: var(--bg-color);
    border-bottom-color: var(--border-light);
  }
  
  .header-left {
    display: flex;
    align-items: center;
    gap: 20px;
    
    .collapse-btn {
      color: var(--text-regular);
      
      &:hover {
        color: var(--primary-color);
        background-color: var(--primary-lighter);
      }
    }
    
    .breadcrumb {
      .breadcrumb-item {
        color: var(--text-regular);
        
        &:last-child {
          color: var(--text-primary);
          font-weight: var(--font-weight-medium);
        }
        
        &:hover {
          color: var(--primary-color);
        }
      }
    }
  }
  
  .header-right {
    display: flex;
    align-items: center;
    gap: 16px;

    .external-links {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-right: 12px;
      padding-right: 12px;
      border-right: 1px solid var(--border-lighter);
    }

    .link-button {
      color: var(--text-regular) !important;
      transition: var(--transition-fast);
      width: 36px;
      height: 36px;
      border-radius: var(--border-radius-full);
      border: 1px solid transparent;

      &:hover {
        color: var(--primary-color) !important;
        background-color: var(--primary-lighter);
        border-color: var(--primary-light);
        transform: scale(1.05);
      }
    }

    .github-btn:hover .github-icon {
      color: #24292e !important;
    }

    .pypi-btn:hover .pypi-icon {
      color: #3775A9 !important;
    }

    .github-icon,
    .pypi-icon {
      width: 18px;
      height: 18px;
      transition: var(--transition-fast);
    }

    .status-badge {
      margin-right: 8px;
    }

    .status-btn,
    .refresh-btn {
      width: 36px;
      height: 36px;
      border-radius: var(--border-radius-full);
      border: 1px solid var(--border-lighter);
      
      &:hover {
        border-color: var(--primary-light);
        transform: scale(1.05);
      }
    }
    
    .theme-switch {
      transform: scale(1.1);
    }
  }
}

// 主内容样式
.main-content {
  padding: 24px;
  overflow-y: auto;
  background: var(--bg-color-page);
  transition: var(--transition-normal);
  
  &.main-content-dark {
    background: var(--bg-color-page);
  }
}

// 动画效果
@keyframes shimmer {
  0% {
    transform: translateX(-100%);
  }
  100% {
    transform: translateX(100%);
  }
}

// 过渡动画
.fade-transform-enter-active,
.fade-transform-leave-active {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.fade-transform-enter-from {
  opacity: 0;
  transform: translateX(30px);
}

.fade-transform-leave-to {
  opacity: 0;
  transform: translateX(-30px);
}

// 响应式设计
@media (max-width: 768px) {
  .sidebar {
    position: fixed;
    left: 0;
    top: 0;
    bottom: 0;
    z-index: 2000;
    transform: translateX(-100%);
    transition: transform var(--transition-normal);
    
    &.sidebar-collapse {
      transform: translateX(-100%);
    }
    
    &:not(.sidebar-collapse) {
      transform: translateX(0);
    }
  }
  
  .main-container {
    margin-left: 0;
  }
  
  .header {
    padding: 0 16px;
    
    .header-right {
      gap: 8px;
      
      .external-links {
        gap: 4px;
        margin-right: 4px;
        padding-right: 4px;
      }
    }
  }
  
  .main-content {
    padding: 16px;
  }
}

// 暗色模式优化
:root.dark {
  .sidebar {
    border-right-color: var(--border-light);
  }
  
  .header {
    border-bottom-color: var(--border-light);
  }
  
  .external-links {
    border-right-color: var(--border-light);
  }
}
</style>
