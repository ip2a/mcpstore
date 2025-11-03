<template>
  <div
    :class="[
      'stat-card',
      cardStyleClass,
      { 'stat-card-clickable': clickable, 'stat-card-loading': loading }
    ]"
    @click="handleClick"
  >
    <!-- 加载状态 -->
    <div v-if="loading" class="stat-card-loader shimmer"></div>

    <!-- 卡片内容 -->
    <div v-else class="stat-card-content">
      <!-- 图标区域 -->
      <div v-if="icon" class="stat-card-icon" :style="iconStyle">
        <component :is="icon" v-if="isComponent" />
        <i v-else :class="icon" :style="{ fontSize: iconSize + 'px' }"></i>
      </div>

      <!-- 主要内容区域 -->
      <div class="stat-card-main">
        <!-- 标题 -->
        <div class="stat-card-title">{{ title }}</div>

        <!-- 数值 -->
        <div class="stat-card-value">
          <span class="value-number">{{ displayValue }}</span>
          <span v-if="unit" class="value-unit">{{ unit }}</span>
        </div>

        <!-- 变化趋势 -->
        <div v-if="trend !== null" class="stat-card-trend" :class="trendClass">
          <el-icon>
            <component :is="trendIcon" />
          </el-icon>
          <span>{{ Math.abs(trend) }}%</span>
          <span class="trend-label">{{ trendLabel }}</span>
        </div>

        <!-- 额外信息 -->
        <div v-if="description" class="stat-card-description">
          {{ description }}
        </div>
      </div>
    </div>

    <!-- 右上角徽章 -->
    <div v-if="badge" class="stat-card-badge">
      <el-badge :value="badge" :type="badgeType" />
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { ArrowUp, ArrowDown, Minus } from '@element-plus/icons-vue'

const props = defineProps({
  // 标题
  title: {
    type: String,
    required: true
  },
  // 数值
  value: {
    type: [Number, String],
    required: true
  },
  // 单位
  unit: {
    type: String,
    default: ''
  },
  // 图标（Element Plus Icon 组件或类名）
  icon: {
    type: [Object, String],
    default: null
  },
  // 图标大小
  iconSize: {
    type: Number,
    default: 32
  },
  // 图标背景色
  iconBgColor: {
    type: String,
    default: null
  },
  // 卡片样式: gradient | glass | plain
  cardStyle: {
    type: String,
    default: 'glass',
    validator: (value) => ['gradient', 'glass', 'plain'].includes(value)
  },
  // 渐变类型（当 cardStyle 为 gradient 时）: primary | success | warning | danger | info | ocean | sunset | forest | sky | fire
  gradientType: {
    type: String,
    default: 'primary',
    validator: (value) => [
      'primary',
      'success',
      'warning',
      'danger',
      'info',
      'ocean',
      'sunset',
      'forest',
      'sky',
      'fire'
    ].includes(value)
  },
  // 变化趋势（百分比）
  trend: {
    type: Number,
    default: null
  },
  // 趋势标签
  trendLabel: {
    type: String,
    default: '较上周'
  },
  // 描述文本
  description: {
    type: String,
    default: ''
  },
  // 徽章
  badge: {
    type: [String, Number],
    default: null
  },
  // 徽章类型
  badgeType: {
    type: String,
    default: 'primary'
  },
  // 是否可点击
  clickable: {
    type: Boolean,
    default: false
  },
  // 加载状态
  loading: {
    type: Boolean,
    default: false
  }
})

const emit = defineEmits(['click'])

// 判断 icon 是否为组件
const isComponent = computed(() => {
  return typeof props.icon === 'object'
})

// 卡片样式类
const cardStyleClass = computed(() => {
  if (props.cardStyle === 'gradient') {
    return `gradient-card-${props.gradientType}`
  } else if (props.cardStyle === 'glass') {
    return 'glass-card'
  }
  return 'stat-card-plain'
})

// 图标样式
const iconStyle = computed(() => {
  const style = {}
  if (props.iconBgColor) {
    style.background = props.iconBgColor
  }
  return style
})

// 格式化显示值
const displayValue = computed(() => {
  if (typeof props.value === 'number') {
    // 格式化数字（添加千分位）
    return props.value.toLocaleString()
  }
  return props.value
})

// 趋势图标
const trendIcon = computed(() => {
  if (props.trend === null || props.trend === 0) return Minus
  return props.trend > 0 ? ArrowUp : ArrowDown
})

// 趋势样式类
const trendClass = computed(() => {
  if (props.trend === null || props.trend === 0) return 'trend-neutral'
  return props.trend > 0 ? 'trend-up' : 'trend-down'
})

// 点击处理
const handleClick = () => {
  if (props.clickable && !props.loading) {
    emit('click')
  }
}
</script>

<style lang="scss" scoped>
.stat-card {
  position: relative;
  padding: var(--spacing-20);
  border-radius: var(--border-radius-xl);
  overflow: hidden;
  transition: var(--transition-base);
  min-height: 140px;

  &-plain {
    background: var(--bg-color);
    border: 1px solid var(--border-lighter);
    box-shadow: var(--shadow-sm);

    &:hover {
      box-shadow: var(--shadow-md);
    }
  }

  &-clickable {
    cursor: pointer;
    user-select: none;

    &:active {
      transform: scale(0.98);
    }
  }

  &-loading {
    pointer-events: none;
  }

  &-loader {
    position: absolute;
    inset: 0;
    border-radius: var(--border-radius-xl);
  }

  &-content {
    display: flex;
    gap: var(--spacing-16);
    align-items: flex-start;
    height: 100%;
  }

  &-icon {
    flex-shrink: 0;
    width: 56px;
    height: 56px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--border-radius-lg);
    background: rgba(255, 255, 255, 0.2);
    color: inherit;
    backdrop-filter: blur(10px);
    transition: var(--transition-base);

    .stat-card:hover & {
      transform: scale(1.1);
    }
  }

  &-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-8);
  }

  &-title {
    font-size: var(--font-size-sm);
    opacity: 0.9;
    font-weight: var(--font-weight-medium);
  }

  &-value {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-4);

    .value-number {
      font-size: var(--font-size-4xl);
      font-weight: var(--font-weight-bold);
      line-height: var(--line-height-tight);
    }

    .value-unit {
      font-size: var(--font-size-base);
      opacity: 0.8;
    }
  }

  &-trend {
    display: inline-flex;
    align-items: center;
    gap: var(--spacing-2);
    font-size: var(--font-size-xs);
    padding: 2px 8px;
    border-radius: var(--border-radius-sm);
    width: fit-content;

    &.trend-up {
      background: rgba(103, 194, 58, 0.2);
      color: var(--success-color);
    }

    &.trend-down {
      background: rgba(245, 108, 108, 0.2);
      color: var(--danger-color);
    }

    &.trend-neutral {
      background: rgba(144, 147, 153, 0.2);
      color: var(--info-color);
    }

    .trend-label {
      margin-left: var(--spacing-2);
      opacity: 0.8;
    }
  }

  &-description {
    font-size: var(--font-size-xs);
    opacity: 0.7;
    margin-top: auto;
  }

  &-badge {
    position: absolute;
    top: var(--spacing-12);
    right: var(--spacing-12);
  }
}

// 渐变卡片特殊样式
[class^='gradient-card-'] {
  .stat-card-title,
  .stat-card-value,
  .stat-card-description {
    color: var(--text-inverse);
  }
}

// 玻璃态卡片特殊样式
.glass-card {
  .stat-card-title {
    color: var(--text-primary);
  }

  .stat-card-value {
    color: var(--text-primary);
  }
}

// 响应式设计
@media (max-width: 768px) {
  .stat-card {
    min-height: 120px;
    padding: var(--spacing-16);

    &-icon {
      width: 48px;
      height: 48px;
    }

    &-value .value-number {
      font-size: var(--font-size-3xl);
    }
  }
}

// 暗色模式适配
:root.dark {
  .stat-card-plain {
    background: var(--bg-color-secondary);
    border-color: var(--border-base);
  }

  .stat-card-icon {
    background: rgba(255, 255, 255, 0.1);
  }
}
</style>
