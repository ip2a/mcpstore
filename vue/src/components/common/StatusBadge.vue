<template>
  <span
    :class="[
      `mcp-status-${status}`,
      sizeClass,
      { 'status-badge-clickable': clickable }
    ]"
    @click="handleClick"
  >
    <slot>{{ displayText }}</slot>
  </span>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
  // MCP 服务状态: initializing | healthy | warning | reconnecting | unreachable | disconnecting | disconnected
  status: {
    type: String,
    required: true,
    validator: (value) => {
      return [
        'initializing',
        'healthy',
        'warning',
        'reconnecting',
        'unreachable',
        'disconnecting',
        'disconnected'
      ].includes(value)
    }
  },
  // 尺寸: small | medium | large
  size: {
    type: String,
    default: 'medium',
    validator: (value) => ['small', 'medium', 'large'].includes(value)
  },
  // 自定义显示文本（如果不提供，使用默认映射）
  text: {
    type: String,
    default: null
  },
  // 是否可点击
  clickable: {
    type: Boolean,
    default: false
  }
})

const emit = defineEmits(['click'])

// 状态文本映射
const statusTextMap = {
  initializing: '初始化中',
  healthy: '健康',
  warning: '警告',
  reconnecting: '重连中',
  unreachable: '不可达',
  disconnecting: '断开中',
  disconnected: '已断开'
}

// 计算显示文本
const displayText = computed(() => {
  return props.text || statusTextMap[props.status] || props.status
})

// 计算尺寸类
const sizeClass = computed(() => {
  return `status-badge-${props.size}`
})

// 点击处理
const handleClick = () => {
  if (props.clickable) {
    emit('click', props.status)
  }
}
</script>

<style lang="scss" scoped>
// 基础样式已在 variables.scss 中定义为 .mcp-status-* 类
// 这里只添加尺寸和交互增强

.status-badge-small {
  padding: 4px 8px;
  font-size: 11px;

  &::before {
    width: 4px;
    height: 4px;
  }
}

.status-badge-medium {
  // 使用 variables.scss 中的默认样式
  // padding: 6px 12px; (已在 @mixin modern-badge 中定义)
}

.status-badge-large {
  padding: 8px 16px;
  font-size: var(--font-size-sm);

  &::before {
    width: 8px;
    height: 8px;
  }
}

.status-badge-clickable {
  cursor: pointer;
  user-select: none;

  &:active {
    transform: translateY(0);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.15);
  }
}

// 暗色模式适配
:root.dark {
  // MCP 状态在暗色模式下的背景色调整
  .mcp-status-initializing {
    background: rgba(64, 158, 255, 0.2);
  }

  .mcp-status-healthy {
    background: rgba(103, 194, 58, 0.2);
  }

  .mcp-status-warning,
  .mcp-status-reconnecting {
    background: rgba(230, 162, 60, 0.2);
  }

  .mcp-status-unreachable {
    background: rgba(245, 108, 108, 0.2);
  }

  .mcp-status-disconnecting,
  .mcp-status-disconnected {
    background: rgba(144, 147, 153, 0.2);
  }
}
</style>
