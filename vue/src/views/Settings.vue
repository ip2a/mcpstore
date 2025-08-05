<template>
  <div class="settings">
    <!-- 页面头部 -->
    <div class="page-header">
      <div class="header-left">
        <h2 class="page-title">系统设置</h2>
        <p class="page-description">配置系统参数和用户偏好</p>
      </div>
      <div class="header-right">
        <el-button 
          type="primary" 
          @click="saveSettings"
          :loading="saving"
        >
          保存设置
        </el-button>
        <el-button 
          @click="resetSettings"
        >
          重置设置
        </el-button>
      </div>
    </div>
    
    <!-- 设置选项卡 -->
    <el-tabs v-model="activeTab" class="settings-tabs">
      <!-- 基础设置 -->
      <el-tab-pane label="基础设置" name="basic">
        <el-card class="settings-card">
          <el-form :model="basicSettings" label-width="150px">
            <el-form-item label="API地址">
              <el-input
                v-model="basicSettings.apiUrl"
                placeholder="http://localhost:18200"
                readonly
              />
              <div class="form-tip">当前API地址（只读）</div>
            </el-form-item>

            <el-form-item label="请求超时">
              <el-input-number
                v-model="basicSettings.timeout"
                :min="5000"
                :max="60000"
                :step="1000"
                style="width: 200px"
              />
              <span class="unit">毫秒</span>
            </el-form-item>

            <el-form-item label="自动刷新">
              <el-switch
                v-model="basicSettings.autoRefresh"
                active-text="启用"
                inactive-text="禁用"
              />
              <div class="form-tip">控制页面数据自动刷新</div>
            </el-form-item>

            <el-form-item label="刷新间隔" v-if="basicSettings.autoRefresh">
              <el-input-number
                v-model="basicSettings.refreshInterval"
                :min="30"
                :max="300"
                :step="30"
                style="width: 200px"
              />
              <span class="unit">秒</span>
            </el-form-item>
          </el-form>
        </el-card>
      </el-tab-pane>
      
      <!-- 界面设置 -->
      <el-tab-pane label="界面设置" name="ui">
        <el-card class="settings-card">
          <el-form :model="uiSettings" label-width="150px">
            <el-form-item label="主题模式">
              <el-radio-group v-model="uiSettings.theme">
                <el-radio label="light">亮色主题</el-radio>
                <el-radio label="dark">暗色主题</el-radio>
              </el-radio-group>
              <div class="form-tip">切换界面主题色调</div>
            </el-form-item>

            <el-form-item label="侧边栏">
              <el-checkbox v-model="uiSettings.sidebarCollapsed">默认收起侧边栏</el-checkbox>
            </el-form-item>

            <el-form-item label="面包屑导航">
              <el-checkbox v-model="uiSettings.showBreadcrumb">显示面包屑导航</el-checkbox>
            </el-form-item>

            <el-form-item label="页面动画">
              <el-checkbox v-model="uiSettings.enableAnimation">启用页面切换动画</el-checkbox>
            </el-form-item>
          </el-form>
        </el-card>
      </el-tab-pane>
      
      <!-- 通知设置 -->
      <el-tab-pane label="通知设置" name="notification">
        <el-card class="settings-card">
          <el-form :model="notificationSettings" label-width="150px">
            <el-form-item label="消息通知">
              <el-checkbox v-model="notificationSettings.showNotifications">显示系统消息</el-checkbox>
              <div class="form-tip">控制页面右上角的消息提示</div>
            </el-form-item>

            <el-form-item label="声音提示">
              <el-checkbox v-model="notificationSettings.sound">启用声音提示</el-checkbox>
              <div class="form-tip">操作完成时播放提示音</div>
            </el-form-item>

            <el-form-item label="通知持续时间">
              <el-input-number
                v-model="notificationSettings.duration"
                :min="2000"
                :max="10000"
                :step="1000"
                style="width: 200px"
              />
              <span class="unit">毫秒</span>
            </el-form-item>
          </el-form>
        </el-card>
      </el-tab-pane>


      <!-- 高级设置 -->
      <el-tab-pane label="高级设置" name="advanced">
        <el-card class="settings-card">
          <el-form :model="advancedSettings" label-width="150px">
            <el-form-item label="调试模式">
              <el-checkbox v-model="advancedSettings.debugMode">启用调试模式</el-checkbox>
              <div class="form-tip">显示详细的调试信息和错误日志</div>
            </el-form-item>

            <el-form-item label="控制台日志">
              <el-checkbox v-model="advancedSettings.consoleLog">显示控制台日志</el-checkbox>
              <div class="form-tip">在浏览器控制台显示详细日志</div>
            </el-form-item>

            <el-form-item label="重试次数">
              <el-input-number
                v-model="advancedSettings.retryCount"
                :min="0"
                :max="5"
                style="width: 200px"
              />
              <span class="unit">次</span>
              <div class="form-tip">API请求失败时的重试次数</div>
            </el-form-item>

            <el-form-item label="页面大小">
              <el-input-number
                v-model="advancedSettings.pageSize"
                :min="10"
                :max="100"
                :step="10"
                style="width: 200px"
              />
              <span class="unit">条/页</span>
              <div class="form-tip">列表页面每页显示的数据条数</div>
            </el-form-item>
          </el-form>
        </el-card>
      </el-tab-pane>
    </el-tabs>
    
    <!-- 系统信息 -->
    <el-card class="info-card">
      <template #header>
        <span>系统信息</span>
      </template>

      <el-descriptions :column="2" border>
        <el-descriptions-item label="前端版本">{{ appStore.config.version }}</el-descriptions-item>
        <el-descriptions-item label="运行环境">{{ appStore.config.environment }}</el-descriptions-item>
        <el-descriptions-item label="前端框架">Vue 3.4 + Element Plus 2.4</el-descriptions-item>
        <el-descriptions-item label="API地址">{{ appStore.config.apiBaseUrl }}</el-descriptions-item>
        <el-descriptions-item label="浏览器">{{ browserInfo }}</el-descriptions-item>
        <el-descriptions-item label="屏幕分辨率">{{ screenResolution }}</el-descriptions-item>
      </el-descriptions>
    </el-card>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useAppStore } from '@/stores/app'
import { ElMessage } from 'element-plus'

const appStore = useAppStore()

// 响应式数据
const activeTab = ref('basic')
const saving = ref(false)

// 设置数据
const basicSettings = ref({
  apiUrl: appStore.config.apiBaseUrl,
  timeout: appStore.config.apiTimeout,
  autoRefresh: appStore.userPreferences.autoRefresh,
  refreshInterval: appStore.userPreferences.refreshInterval / 1000 // 转换为秒
})

const uiSettings = ref({
  theme: appStore.currentTheme,
  sidebarCollapsed: appStore.isCollapse,
  showBreadcrumb: appStore.layoutConfig.showBreadcrumb,
  enableAnimation: appStore.userPreferences.animationEnabled
})

const notificationSettings = ref({
  showNotifications: appStore.userPreferences.showNotifications,
  sound: appStore.userPreferences.soundEnabled,
  duration: 4500
})

const advancedSettings = ref({
  debugMode: appStore.config.environment === 'development',
  consoleLog: appStore.config.environment === 'development',
  retryCount: 2,
  pageSize: appStore.userPreferences.pageSize
})

// 计算属性
const browserInfo = computed(() => {
  const ua = navigator.userAgent
  if (ua.includes('Chrome')) return 'Chrome'
  if (ua.includes('Firefox')) return 'Firefox'
  if (ua.includes('Safari')) return 'Safari'
  if (ua.includes('Edge')) return 'Edge'
  return 'Unknown'
})

const screenResolution = computed(() => {
  return `${screen.width} × ${screen.height}`
})

// 方法
const saveSettings = async () => {
  saving.value = true
  try {
    // 应用基础设置
    appStore.userPreferences.autoRefresh = basicSettings.value.autoRefresh
    appStore.userPreferences.refreshInterval = basicSettings.value.refreshInterval * 1000 // 转换为毫秒

    // 应用UI设置
    appStore.setTheme(uiSettings.value.theme)
    appStore.setCollapse(uiSettings.value.sidebarCollapsed)
    appStore.layoutConfig.showBreadcrumb = uiSettings.value.showBreadcrumb
    appStore.userPreferences.animationEnabled = uiSettings.value.enableAnimation

    // 应用通知设置
    appStore.userPreferences.showNotifications = notificationSettings.value.showNotifications
    appStore.userPreferences.soundEnabled = notificationSettings.value.sound

    // 应用高级设置
    appStore.userPreferences.pageSize = advancedSettings.value.pageSize

    // 保存到localStorage
    appStore.saveSettings()

    ElMessage.success('设置保存成功')
  } catch (error) {
    console.error('保存设置失败:', error)
    ElMessage.error('设置保存失败')
  } finally {
    saving.value = false
  }
}

const resetSettings = () => {
  // 重置为默认值
  basicSettings.value = {
    apiUrl: appStore.config.apiBaseUrl,
    timeout: 15000,
    autoRefresh: false,
    refreshInterval: 60
  }

  uiSettings.value = {
    theme: 'light',
    sidebarCollapsed: false,
    showBreadcrumb: true,
    enableAnimation: true
  }

  notificationSettings.value = {
    showNotifications: true,
    sound: false,
    duration: 4500
  }

  advancedSettings.value = {
    debugMode: false,
    consoleLog: false,
    retryCount: 2,
    pageSize: 20
  }

  // 应用重置的设置
  appStore.resetSettings()

  ElMessage.success('设置已重置为默认值')
}

const loadSettings = () => {
  try {
    // 从store中加载当前设置
    basicSettings.value = {
      apiUrl: appStore.config.apiBaseUrl,
      timeout: appStore.config.apiTimeout,
      autoRefresh: appStore.userPreferences.autoRefresh,
      refreshInterval: appStore.userPreferences.refreshInterval / 1000
    }

    uiSettings.value = {
      theme: appStore.currentTheme,
      sidebarCollapsed: appStore.isCollapse,
      showBreadcrumb: appStore.layoutConfig.showBreadcrumb,
      enableAnimation: appStore.userPreferences.animationEnabled
    }

    notificationSettings.value = {
      showNotifications: appStore.userPreferences.showNotifications,
      sound: appStore.userPreferences.soundEnabled,
      duration: 4500
    }

    advancedSettings.value = {
      debugMode: appStore.config.environment === 'development',
      consoleLog: appStore.config.environment === 'development',
      retryCount: 2,
      pageSize: appStore.userPreferences.pageSize
    }
  } catch (error) {
    console.warn('Failed to load settings:', error)
  }
}

// 生命周期
onMounted(() => {
  loadSettings()
})
</script>

<style lang="scss" scoped>
.settings {
  .page-header {
    @include flex-between;
    margin-bottom: 20px;
    
    .header-left {
      .page-title {
        margin: 0 0 4px 0;
        font-size: 24px;
        font-weight: var(--font-weight-medium);
      }
      
      .page-description {
        margin: 0;
        color: var(--text-secondary);
      }
    }
    
    .header-right {
      display: flex;
      gap: 12px;
    }
  }
  
  .settings-tabs {
    margin-bottom: 20px;
    
    .settings-card {
      min-height: 400px;
      
      .unit {
        margin-left: 8px;
        color: var(--text-secondary);
        font-size: var(--font-size-sm);
      }

      .form-tip {
        margin-top: 4px;
        font-size: 12px;
        color: var(--el-text-color-secondary);
        line-height: 1.4;
      }
    }
  }
  
  .info-card {
    margin-top: 20px;
  }
}

// 响应式适配
@include respond-to(xs) {
  .settings {
    .page-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 16px;
      
      .header-right {
        width: 100%;
        justify-content: flex-end;
      }
    }
  }
}
</style>
