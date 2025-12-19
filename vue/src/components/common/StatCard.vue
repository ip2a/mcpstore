<template>
  <div
    class="stat-card"
    :class="{ 'clickable': clickable }"
    @click="handleClick"
  >
    <!-- 顶部：标题和图标 -->
    <div class="card-header">
      <span class="card-title">{{ title }}</span>
      <div
        v-if="icon"
        class="card-icon"
      >
        <component
          :is="icon"
          v-if="isComponent"
        />
        <i
          v-else
          :class="icon"
        />
      </div>
    </div>

    <!-- 中部：核心数值 -->
    <div class="card-body">
      <div class="value-container">
        <span class="value">{{ displayValue }}</span>
        <span
          v-if="unit"
          class="unit"
        >{{ unit }}</span>
      </div>
    </div>

    <!-- 底部：趋势或描述 (仅文字，极简) -->
    <div
      v-if="hasTrend || description"
      class="card-footer"
    >
      <div
        v-if="hasTrend"
        class="trend"
        :class="trendClass"
      >
        <component
          :is="trendIcon"
          class="trend-icon"
        />
        <span>{{ Math.abs(trend) }}%</span>
      </div>
      <span
        v-if="description"
        class="description"
      >{{ description }}</span>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { ArrowUp, ArrowDown, Minus } from '@element-plus/icons-vue'

const props = defineProps({
  title: String,
  value: [Number, String],
  unit: String,
  icon: [Object, String],
  description: String,
  trend: Number,
  clickable: Boolean
})

const emit = defineEmits(['click'])

const isComponent = computed(() => typeof props.icon === 'object')
const hasTrend = computed(() => typeof props.trend === 'number')

const displayValue = computed(() => {
  return typeof props.value === 'number' ? props.value.toLocaleString() : props.value
})

const trendIcon = computed(() => {
  if (!props.trend) return Minus
  return props.trend > 0 ? ArrowUp : ArrowDown
})

const trendClass = computed(() => {
  if (!props.trend) return 'neutral'
  return props.trend > 0 ? 'up' : 'down'
})

const handleClick = () => {
  if (props.clickable) emit('click')
}
</script>

<style lang="scss" scoped>
.stat-card {
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: 16px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  height: 100%;
  min-height: 110px;
  transition: all 0.2s ease;
  
  // Force removal of any shadow
  box-shadow: none !important;
  filter: none !important;

  // Non-clickable hover: subtle border change only
  &:hover {
    border-color: var(--border-color-dark);
  }

  // Clickable hover: slightly more pronounced
  &.clickable {
    cursor: pointer;
    
    &:hover {
      border-color: var(--text-secondary);
      background-color: var(--bg-hover);
      transform: translateY(-1px);
    }
    
    &:active {
      transform: translateY(0);
      background-color: var(--bg-active);
    }
  }
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 8px;

  .card-title {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-secondary); // 弱化标题
  }

  .card-icon {
    color: var(--text-primary); // 图标使用主色，不再使用彩色背景
    font-size: 16px;
    opacity: 0.8;
  }
}

.card-body {
  .value-container {
    display: flex;
    align-items: baseline;
    gap: 4px;
  }

  .value {
    font-size: 24px;
    font-weight: 600;
    color: var(--text-primary);
    line-height: 1.2;
    letter-spacing: -0.02em;
  }

  .unit {
    font-size: 13px;
    color: var(--text-secondary);
    font-weight: 400;
  }
}

.card-footer {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
  font-size: 12px;

  .trend {
    display: flex;
    align-items: center;
    gap: 2px;
    font-weight: 500;
    
    &.up { color: var(--color-success); }
    &.down { color: var(--color-danger); }
    &.neutral { color: var(--text-secondary); }

    .trend-icon {
      width: 12px;
      height: 12px;
    }
  }

  .description {
    color: var(--text-placeholder);
  }
}
</style>