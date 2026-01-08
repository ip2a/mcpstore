<template>
  <div class="agent-service-add-page">
    <header class="page-header">
      <div class="header-content">
        <h1 class="page-title">
          Add Service
        </h1>
        <p class="page-subtitle">
          统一的服务注册表单，支持绑定 Agent 或直接添加到 Store
        </p>
      </div>
      <div class="header-actions">
        <el-button
          link
          class="back-link"
          @click="$router.back()"
        >
          <el-icon><ArrowLeft /></el-icon> 返回
        </el-button>
      </div>
    </header>

    <ServiceForm
      class="form-wrapper"
      :default-agent-id="targetAgentId"
      @success="handleSuccess"
    />
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ArrowLeft } from '@element-plus/icons-vue'
import ServiceForm from '@/components/agents/ServiceForm.vue'

const route = useRoute()
const router = useRouter()
const targetAgentId = computed(() => route.query.agentId || route.params.agent_id || '')

const handleSuccess = (payload) => {
  if (payload?.scope === 'agent' && payload.agentId) {
    router.push({ name: 'for_store_agent_detail', params: { id: payload.agentId } })
  } else {
    router.push({ name: 'for_store_list_services' })
  }
}
</script>

<style scoped lang="scss">
.agent-service-add-page {
  max-width: 1200px;
  margin: 0 auto;
  padding: 20px;
  width: 100%;
}

.page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  margin-bottom: 24px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border-color);
}

.page-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 4px;
}

.page-subtitle {
  font-size: 13px;
  color: var(--text-secondary);
}

.back-link {
  color: var(--text-secondary);
  font-size: 13px;
  &:hover { color: var(--text-primary); }
}
</style>
