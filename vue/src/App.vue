<template>
  <div id="app">
    <!-- 主布局 -->
    <el-container style="height: 100vh">
      <!-- 侧边栏：极简线框风格 -->
      <el-aside
        width="240px"
        class="app-aside"
      >
        <div class="brand">
          <div class="logo-circle">
            M
          </div>
          <span class="brand-text">MCPStore</span>
        </div>

        <el-menu
          :default-active="$route.path"
          router
          class="atom-menu"
        >
          <!-- 核心功能组 -->
          <div class="menu-group-label">
            Core
          </div>
          
          <el-menu-item index="/system/dashboard">
            <el-icon><Monitor /></el-icon>
            <span>Dashboard</span>
          </el-menu-item>

          <el-menu-item index="/for_store/list_services">
            <el-icon><Connection /></el-icon>
            <span>Services</span>
          </el-menu-item>

          <el-menu-item index="/for_store/list_tools">
            <el-icon><Tools /></el-icon>
            <span>Tools</span>
          </el-menu-item>

          <!-- 操作组 -->
          <div class="menu-group-label">
            Operations
          </div>

          <el-menu-item index="/for_store/add_service">
            <el-icon><Plus /></el-icon>
            <span>Add Service</span>
          </el-menu-item>
          
          <el-menu-item index="/for_store/call_tool">
            <el-icon><VideoPlay /></el-icon>
            <span>Execute Tool</span>
          </el-menu-item>
          
          <el-menu-item index="/for_store/tool_records">
            <el-icon><Document /></el-icon>
            <span>History</span>
          </el-menu-item>

          <!-- 系统组 -->
          <div class="menu-group-label">
            System
          </div>

          <el-menu-item index="/for_store/list_agents">
            <el-icon><User /></el-icon>
            <span>Agents</span>
          </el-menu-item>

          <el-menu-item index="/for_store/show_config">
            <el-icon><Setting /></el-icon>
            <span>Configuration</span>
          </el-menu-item>
          
          <el-menu-item index="/for_store/show_cache">
            <el-icon><Coin /></el-icon>
            <span>Cache</span>
          </el-menu-item>

          <!-- 外部链接 -->
          <div class="menu-group-label">
            External
          </div>
           
          <el-menu-item index="/docs">
            <el-icon><Reading /></el-icon>
            <span>Docs</span>
          </el-menu-item>
          
          <el-menu-item index="/external/github">
            <el-icon><Link /></el-icon>
            <span>GitHub</span>
          </el-menu-item>
        </el-menu>
        
        <!-- 底部用户/状态区 (可选) -->
        <div class="aside-footer">
          <div class="status-indicator is-active" />
          <span class="text-xs text-secondary">System Online</span>
        </div>
      </el-aside>


      <!-- 主内容区 -->
      <el-container class="main-container">
        <!-- 顶部导航栏：仅保留面包屑或极简标题，与 Tab 融合 -->
        <el-header
          height="50px"
          class="app-header"
        >
          <!-- 左侧：面包屑或当前路径 -->
          <div class="header-breadcrumbs">
            <span class="text-secondary">MCPStore</span>
            <span class="separator">/</span>
            <span class="text-primary">{{ currentPageTitle }}</span>
          </div>
           
          <!-- 右侧：全局操作 -->
          <div class="header-actions">
            <button
              class="icon-btn"
              :class="{ spinning: isRefreshing }"
              @click="refreshData"
            >
              <el-icon><Refresh /></el-icon>
            </button>
            <button
              class="icon-btn"
              @click="toggleTheme"
            >
              <el-icon v-if="isDark">
                <Sunny />
              </el-icon>
              <el-icon v-else>
                <Moon />
              </el-icon>
            </button>
          </div>
        </el-header>

        <!-- 标签页（极简版） -->
        <div class="tabs-wrap">
          <TabsView />
        </div>

        <!-- 主内容 -->
        <el-main class="app-main">
          <router-view v-slot="{ Component, route }">
            <transition
              name="fade-transform"
              mode="out-in"
            >
              <keep-alive :include="route.meta?.keepAlive ? [route.name] : []">
                <component
                  :is="Component"
                  :key="route.path"
                />
              </keep-alive>
            </transition>
          </router-view>
        </el-main>
      </el-container>
    </el-container>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import TabsView from '@/components/layout/TabsView.vue'
import { useAppStore } from '@/stores/app'
import { useSystemStore } from '@/stores/system'
import { useServicesStore } from '@/stores/services'
import { useToolsStore } from '@/stores/tools'
import {
  Monitor, Connection, Tools, User, Document, Setting, 
  SuccessFilled, WarningFilled, Refresh, Moon, Sunny, Plus, Link, VideoPlay, Reading, Coin
} from '@element-plus/icons-vue'

const route = useRoute()
const appStore = useAppStore()
const systemStore = useSystemStore()
const servicesStore = useServicesStore()
const toolsStore = useToolsStore()

// 响应式数据
const isDark = computed({
  get: () => appStore.isDark,
  set: (value) => appStore.setTheme(value ? 'dark' : 'light')
})

const isRefreshing = computed(() => appStore.isLoading || systemStore.isLoading)

const currentPageTitle = computed(() => route.meta?.title || 'MCPStore')

// 方法
const toggleTheme = () => {
  const newTheme = appStore.isDark ? 'light' : 'dark'
  appStore.setTheme(newTheme)
  document.documentElement.classList.toggle('dark', newTheme === 'dark')
}

const refreshData = async () => {
  try {
    appStore.setGlobalLoading(true)
    await Promise.all([
      systemStore.refreshAllData(),
      servicesStore.refreshAll(),
      toolsStore.fetchTools(true)
    ])
  } catch (error) {
    console.error('刷新数据失败:', error)
  } finally {
    appStore.setGlobalLoading(false)
  }
}
</script>

<style lang="scss" scoped>
// 布局基础
.app-aside {
  background: var(--bg-surface);
  border-right: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
}

.main-container {
  background: var(--bg-body);
}

// 侧边栏品牌
.brand {
  height: 60px;
  display: flex;
  align-items: center;
  padding: 0 24px;
  border-bottom: 1px solid var(--border-color);
  gap: 12px;
}

.logo-circle {
  width: 24px;
  height: 24px;
  background: var(--color-primary);
  color: #fff;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 700;
  font-size: 14px;
}

.brand-text {
  font-weight: 600;
  font-size: 15px;
  letter-spacing: -0.01em;
  color: var(--text-primary);
}

// 侧边栏菜单 (自定义覆盖 Element Plus)
.atom-menu {
  border-right: none;
  background: transparent;
  padding: 16px 12px;
  flex: 1;
}

.menu-group-label {
  padding: 16px 12px 8px;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--text-placeholder);
  letter-spacing: 0.05em;
  
  &:first-child { padding-top: 0; }
}

:deep(.el-menu-item) {
  height: 36px;
  line-height: 36px;
  border-radius: 6px;
  margin-bottom: 2px;
  color: var(--text-secondary);
  font-size: 13px;
  padding: 0 12px !important; // Override element style
  
  &:hover {
    background-color: var(--bg-hover);
    color: var(--text-primary);
  }
  
  &.is-active {
    background-color: var(--bg-active);
    color: var(--text-primary);
    font-weight: 500;
  }
  
  .el-icon {
    font-size: 16px;
    margin-right: 10px;
    color: inherit;
  }
}

.aside-footer {
  padding: 16px 24px;
  border-top: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-indicator {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  
  &.is-active {
    background-color: var(--color-success);
    box-shadow: 0 0 0 2px rgba(16, 185, 129, 0.2); // 仅此处保留微弱光晕
  }
}

// 顶部 Header
.app-header {
  background: var(--bg-body); // 与背景同色，去边框
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 20px;
  // border-bottom: 1px solid var(--border-color); // 可选：移除顶部边框，更极简
}

.header-breadcrumbs {
  display: flex;
  align-items: center;
  font-size: 13px;
  
  .separator {
    margin: 0 8px;
    color: var(--border-color-dark);
  }
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.icon-btn {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 6px;
  color: var(--text-secondary);
  transition: all 0.2s;
  
  &:hover {
    background-color: var(--bg-hover);
    color: var(--text-primary);
  }
  
  &.spinning {
    animation: spin 1s linear infinite;
  }
}

// Tabs
.tabs-wrap {
  padding: 0 20px;
  margin-bottom: 16px;
}

// Main Content
.app-main {
  padding: 0; // Remove default padding
  position: relative;
  overflow-x: hidden;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

/* 路由过渡动画 */
.fade-transform-enter-active,
.fade-transform-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}

.fade-transform-enter-from {
  opacity: 0;
  transform: translateY(4px);
}

.fade-transform-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>