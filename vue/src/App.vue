<template>
  <div id="app">
    <!-- 主布局 -->
    <el-container style="height: 100vh">
      <!-- 侧边栏 -->
      <el-aside width="200px" class="app-aside">
        <div class="brand">
          <span class="brand-text">MCPStore</span>
        </div>

        <el-menu
          :default-active="$route.path"
          router
        >
          <!-- 仪表板 -->
          <el-menu-item index="/system/dashboard">
            <el-icon><Monitor /></el-icon>
            <span>仪表板</span>
          </el-menu-item>

          <!-- 服务列表 -->
          <el-menu-item index="/for_store/list_services">
            <el-icon><Connection /></el-icon>
            <span>服务列表</span>
          </el-menu-item>

          <!-- 添加服务 -->
          <el-menu-item index="/for_store/add_service">
            <el-icon><Plus /></el-icon>
            <span>添加服务</span>
          </el-menu-item>

          <!-- 工具列表 -->
          <el-menu-item index="/for_store/list_tools">
            <el-icon><Tools /></el-icon>
            <span>工具列表</span>
          </el-menu-item>

          <!-- 工具执行 -->
          <el-menu-item index="/for_store/call_tool">
            <el-icon><VideoPlay /></el-icon>
            <span>工具执行</span>
          </el-menu-item>

          <!-- 工具记录 -->
          <el-menu-item index="/for_store/tool_records">
            <el-icon><Document /></el-icon>
            <span>工具记录</span>
          </el-menu-item>

          <!-- Agent列表 -->
          <el-menu-item index="/for_store/list_agents">
            <el-icon><User /></el-icon>
            <span>Agent列表</span>
          </el-menu-item>

          <!-- 配置中心 -->
          <el-menu-item index="/for_store/show_config">
            <el-icon><Setting /></el-icon>
            <span>配置中心</span>
          </el-menu-item>

          <!-- 文档中心 -->
          <el-menu-item index="/docs">
            <el-icon><Reading /></el-icon>
            <span>文档中心</span>
          </el-menu-item>

          <!-- 外链：GitHub 项目 -->
          <el-menu-item index="/external/github">
            <el-icon><Link /></el-icon>
            <span>GitHub 项目</span>
          </el-menu-item>

          <!-- 外链：PyPI 页面 -->
          <el-menu-item index="/external/pypi">
            <el-icon><Link /></el-icon>
            <span>PyPI 页面</span>
          </el-menu-item>

          <!-- 缓存空间 -->
          <el-menu-item index="/for_store/show_cache">
            <el-icon><Coin /></el-icon>
            <span>缓存空间</span>
          </el-menu-item>
        </el-menu>
      </el-aside>


      <!-- 主内容区 -->
      <el-container>
        <!-- 顶部导航栏 -->
        <el-header height="60px" class="app-header">
          <div class="header-left">
            <h3 class="page-title">{{ currentPageTitle }}</h3>
          </div>

          <div class="header-right"></div>
        </el-header>

        <!-- 标签页（简洁版） -->
        <div class="tabs-wrap">
          <TabsView />
        </div>

        <!-- 主内容 -->
        <el-main class="app-main">
          <router-view v-slot="{ Component, route }">
            <transition name="fade-transform" mode="out-in">
              <keep-alive :include="route.meta?.keepAlive ? [route.name] : []">
                <component :is="Component" :key="route.path" />
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
  Monitor, Connection, Tools, User, Document, Setting, FolderOpened,
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

const systemStatus = computed(() => systemStore.systemStatus)

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

const checkSystemStatus = async () => {
  await systemStore.safeCheckSystemStatus()
}

const openGitHub = () => {
  const url = appStore.config.githubUrl || 'https://github.com/whillhill/mcpstore'
  window.open(url, '_blank')
}

const openPyPI = () => {
  const url = appStore.config.pypiUrl || 'https://pypi.org/project/mcpstore'
  window.open(url, '_blank')
}

</script>

<style scoped>
/* 统一的布局边界与留白，遵循 Element Plus 设计变量 */
.app-aside {
  background: var(--el-bg-color);
  border-right: 1px solid var(--el-border-color-light);
}
.brand {
  padding: 16px;
  text-align: center;
  border-bottom: 1px solid var(--el-border-color-light);
}
.brand-text {
  font-weight: 600;
  font-size: 18px;
  color: var(--el-text-color-primary);
}
.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px;
  border-bottom: 1px solid var(--el-border-color-light);
  background: var(--el-bg-color);
}
.page-title { margin: 0; font-weight: 600; color: var(--el-text-color-primary); }
.tabs-wrap {
  background: var(--el-bg-color);
  border-bottom: 1px solid var(--el-border-color-light);
}
.app-main {
  padding: 16px;
  background: var(--el-bg-color-page, #f5f7fa);
  position: relative;
  overflow: auto;
}

/* 路由过渡动画 - 轻量级淡入淡出 */
.fade-transform-enter-active,
.fade-transform-leave-active {
  transition: opacity 0.15s ease-in-out;
}

.fade-transform-enter-from {
  opacity: 0;
}

.fade-transform-leave-to {
  opacity: 0;
}

.fade-transform-enter-to,
.fade-transform-leave-from {
  opacity: 1;
}
</style>
