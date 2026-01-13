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
import { formatStatusText } from '@/utils/serviceStatus'

const props = defineProps({
  // MCP 服务状态（使用新 8 态枚举，未知值回退为原文）
  status: {
    type: String,
    required: true
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

// 计算显示文本
const displayText = computed(() => props.text || formatStatusText(props.status))

// 计算尺寸类
const sizeClass = computed(() => `status-badge-${props.size}`)

// 点击处理
const handleClick = () => {
  if (props.clickable) emit('click', props.status)
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

.mcp-status-init,
.mcp-status-startup {
  background: #e5f0ff;
  color: #1d4ed8;
  border: 1px solid #c7d7ff;
}

.mcp-status-ready {
  background: #e0f2fe;
  color: #0369a1;
  border: 1px solid #bae6fd;
}

.mcp-status-healthy {
  background: #ecfdf3;
  color: #15803d;
  border: 1px solid #bbf7d0;
}

.mcp-status-degraded,
.mcp-status-half-open {
  background: #fff7ed;
  color: #c2410c;
  border: 1px solid #fed7aa;
}

.mcp-status-circuit-open {
  background: #fef2f2;
  color: #b91c1c;
  border: 1px solid #fecdd3;
}

.mcp-status-disconnected {
  background: #f3f4f6;
  color: #4b5563;
  border: 1px solid #e5e7eb;
}

.mcp-status-unknown {
  background: #f3f4f6;
  color: #6b7280;
  border: 1px solid #e5e7eb;
}

// 暗色模式适配
:root.dark {
  .mcp-status-init,
  .mcp-status-startup {
    background: rgba(59, 130, 246, 0.2);
  }

  .mcp-status-ready {
    background: rgba(14, 165, 233, 0.2);
  }

  .mcp-status-healthy {
    background: rgba(16, 185, 129, 0.2);
  }

  .mcp-status-degraded,
  .mcp-status-half-open {
    background: rgba(234, 179, 8, 0.2);
  }

  .mcp-status-circuit-open {
    background: rgba(239, 68, 68, 0.2);
  }

  .mcp-status-disconnected,
  .mcp-status-unknown {
    background: rgba(107, 114, 128, 0.2);
  }
}
</style>
