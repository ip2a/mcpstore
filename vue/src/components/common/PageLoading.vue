<template>
  <div v-if="loading" class="page-loading-overlay" :class="{ 'full-screen': fullScreen }">
    <div class="loading-content">
      <el-icon class="loading-icon" :size="size">
        <Loading />
      </el-icon>
      <div v-if="text" class="loading-text">{{ text }}</div>
    </div>
  </div>
</template>

<script setup>
import { Loading } from '@element-plus/icons-vue'

defineProps({
  loading: {
    type: Boolean,
    default: false
  },
  text: {
    type: String,
    default: '加载中...'
  },
  fullScreen: {
    type: Boolean,
    default: false
  },
  size: {
    type: [String, Number],
    default: 32
  }
})
</script>

<style lang="scss" scoped>
.page-loading-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(255, 255, 255, 0.8);
  backdrop-filter: blur(2px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  border-radius: inherit;
  
  &.full-screen {
    position: fixed;
    background-color: rgba(255, 255, 255, 0.9);
    z-index: 9999;
  }
  
  .loading-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    
    .loading-icon {
      color: var(--el-color-primary);
      animation: rotate 1s linear infinite;
    }
    
    .loading-text {
      font-size: 14px;
      color: var(--el-text-color-regular);
      font-weight: 500;
    }
  }
}

@keyframes rotate {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

// 暗色模式适配
.dark .page-loading-overlay {
  background-color: rgba(0, 0, 0, 0.8);
  
  &.full-screen {
    background-color: rgba(0, 0, 0, 0.9);
  }
}
</style>
