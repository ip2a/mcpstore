<template>
  <div class="external-link-page">
    <el-card class="link-card" shadow="hover">
      <div class="link-content">
        <!-- 图标 -->
        <div class="link-icon">
          <el-icon :size="80">
            <component :is="iconComponent" />
          </el-icon>
        </div>

        <!-- 标题 -->
        <h2 class="link-title">{{ title }}</h2>

        <!-- 描述 -->
        <p class="link-description">{{ description }}</p>

        <!-- URL 显示 -->
        <div class="link-url">
          <el-icon><Link /></el-icon>
          <span>{{ url }}</span>
        </div>

        <!-- 操作按钮 -->
        <div class="link-actions">
          <el-button type="primary" size="large" @click="openInNewTab">
            <el-icon class="mr-2"><TopRight /></el-icon>
            Open in New Tab
          </el-button>
          <el-button size="large" @click="copyLink">
            <el-icon class="mr-2"><CopyDocument /></el-icon>
            Copy Link
          </el-button>
        </div>

        <!-- 提示信息 -->
        <div class="link-note">
          <el-icon><InfoFilled /></el-icon>
          <span>This external site cannot be embedded due to security policies.</span>
        </div>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { ElMessage } from 'element-plus'
import { Link, TopRight, CopyDocument, InfoFilled } from '@element-plus/icons-vue'
import * as Icons from '@element-plus/icons-vue'

const route = useRoute()

const title = computed(() => route.meta?.title || 'External Link')
const url = computed(() => route.meta?.url || '')
const description = computed(() => route.meta?.description || 'Click below to open this external resource.')
const icon = computed(() => route.meta?.icon || 'Link')

// 动态获取图标组件
const iconComponent = computed(() => {
  return Icons[icon.value] || Icons.Link
})

const openInNewTab = () => {
  if (url.value) {
    window.open(url.value, '_blank', 'noopener,noreferrer')
    ElMessage.success('Opening in new tab...')
  }
}

const copyLink = async () => {
  if (url.value) {
    try {
      await navigator.clipboard.writeText(url.value)
      ElMessage.success('Link copied to clipboard!')
    } catch (error) {
      // 降级方案：使用传统方法
      const textArea = document.createElement('textarea')
      textArea.value = url.value
      textArea.style.position = 'fixed'
      textArea.style.opacity = '0'
      document.body.appendChild(textArea)
      textArea.select()
      try {
        document.execCommand('copy')
        ElMessage.success('Link copied to clipboard!')
      } catch (err) {
        ElMessage.error('Failed to copy link')
      }
      document.body.removeChild(textArea)
    }
  }
}
</script>

<style scoped>
.external-link-page {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: calc(100vh - 200px);
  padding: 40px 20px;
}

.link-card {
  max-width: 600px;
  width: 100%;
}

.link-content {
  text-align: center;
  padding: 40px 20px;
}

.link-icon {
  margin-bottom: 24px;
  color: var(--el-color-primary);
  display: flex;
  justify-content: center;
}

.link-title {
  margin: 0 0 16px 0;
  font-size: 28px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.link-description {
  margin: 0 0 24px 0;
  font-size: 16px;
  color: var(--el-text-color-secondary);
  line-height: 1.6;
}

.link-url {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 12px 20px;
  background: var(--el-fill-color-light);
  border-radius: 8px;
  margin-bottom: 32px;
  font-family: 'Courier New', monospace;
  font-size: 14px;
  color: var(--el-color-primary);
  word-break: break-all;
}

.link-actions {
  display: flex;
  gap: 12px;
  justify-content: center;
  margin-bottom: 24px;
  flex-wrap: wrap;
}

.link-note {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  font-size: 13px;
  color: var(--el-text-color-secondary);
  padding: 12px;
  background: var(--el-fill-color-extra-light);
  border-radius: 6px;
  border-left: 3px solid var(--el-color-info);
}

.mr-2 {
  margin-right: 4px;
}

/* 响应式 */
@media (max-width: 768px) {
  .link-title {
    font-size: 24px;
  }

  .link-description {
    font-size: 14px;
  }

  .link-actions {
    flex-direction: column;
  }

  .link-actions .el-button {
    width: 100%;
  }
}
</style>

