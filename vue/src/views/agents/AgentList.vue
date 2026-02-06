<template>
  <div class="page-shell agent-list-container">
    <!-- Header -->
    <header class="page-header">
      <div class="header-content">
        <h1 class="page-title">
          Agents
        </h1>
        <p class="page-subtitle">
          Manage isolated agent contexts and their services
        </p>
      </div>
      <div class="header-actions">
        <el-input
          v-model="searchQuery"
          placeholder="搜索 Agent ID 或描述"
          clearable
          class="search-input"
          :prefix-icon="Search"
        />
        <el-button 
          :icon="Plus" 
          type="primary"
          color="#000"
          class="create-btn"
          @click="openServiceDialog()"
        >
          Create Service
        </el-button>
        <el-button 
          :icon="Refresh" 
          :loading="agentsStore.loading"
          circle
          plain
          class="refresh-btn"
          @click="refreshAgents"
        />
      </div>
    </header>

    <!-- Content -->
    <div class="main-content">
      <!-- Empty State -->
      <div
        v-if="!agentsStore.loading && agentsStore.agents.length === 0"
        class="empty-state"
      >
        <el-icon class="empty-icon">
          <User />
        </el-icon>
        <h3>No Agents Found</h3>
        <p>Create your first service to initialize an agent context.</p>
        <el-button @click="$router.push('/for_store/add_service')">
          Add Service
        </el-button>
      </div>

      <!-- Agent Grid -->
      <div
        v-else
        class="agent-grid"
      >
        <div 
          v-for="agent in filteredAgents" 
          :key="agent.id" 
          class="agent-card"
          @click="viewAgentDetails(agent)"
        >
          <div class="card-header">
            <div class="header-main">
              <h3 class="agent-name">
                {{ agent.name }}
              </h3>
              <span class="agent-id">ID: {{ agent.id }}</span>
            </div>
            <span
              class="status-indicator"
              :class="getStatusClass(agent.status)"
            />
          </div>
            
          <div class="card-body">
            <p class="description">
              {{ agent.description || 'No description provided.' }}
            </p>
               
            <div class="stats-row">
              <div class="stat">
                <span class="val">{{ agent.services || 0 }}</span>
                <span class="lbl">Services</span>
              </div>
              <div class="stat">
                <span class="val">{{ agent.tools || 0 }}</span>
                <span class="lbl">Tools</span>
              </div>
              <div class="stat">
                <span class="val success">{{ agent.healthy_services || 0 }}</span>
                <span class="lbl">Healthy</span>
              </div>
              <div class="stat">
                <span class="val error">{{ agent.unhealthy_services || 0 }}</span>
                <span class="lbl">Issues</span>
              </div>
            </div>
               
            <div class="last-active">
              Last Active: {{ formatTime(agent.last_activity) }}
            </div>
          </div>
            
          <div class="card-footer">
            <button
              class="text-btn"
              @click.stop="addServiceToAgent(agent)"
            >
              Add Service
            </button>
            <button
              class="text-btn"
              @click.stop="manageAgent(agent)"
            >
              Manage
            </button>
            <button
              class="text-btn danger"
              @click.stop="resetAgent(agent)"
            >
              Reset
            </button>
          </div>
        </div>
      </div>
    </div>

    <el-dialog
      v-model="serviceDialogVisible"
      width="1000px"
      class="agent-service-dialog"
      destroy-on-close
      :close-on-click-modal="false"
      title="Add Service"
    >
      <ServiceForm
        :default-agent-id="dialogAgentId"
        compact
        @success="handleServiceAdded"
      />
    </el-dialog>
  </div>
</template>

<script setup>
import { onMounted, ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import dayjs from 'dayjs'
import { Plus, Refresh, User, Search } from '@element-plus/icons-vue'
import { useAgentsStore } from '@/stores/agents'
import ServiceForm from '@/components/agents/ServiceForm.vue'

const router = useRouter()
const agentsStore = useAgentsStore()
const searchQuery = ref('')
const serviceDialogVisible = ref(false)
const dialogAgentId = ref('')

const getStatusClass = (status) => {
  switch (status) {
    case 'active': return 'active'
    case 'partial': return 'warn'
    case 'error': return 'error'
    default: return 'inactive'
  }
}

const refreshAgents = async () => {
  try {
    await agentsStore.fetchAgents()
    ElMessage.success('Refreshed')
  } catch (e) {
    ElMessage.error('Failed to refresh')
  }
}

const viewAgentDetails = (agent) => {
  router.push({ name: 'for_store_agent_detail', params: { id: agent.id } })
}

const addServiceToAgent = (agent) => {
  dialogAgentId.value = agent.id
  serviceDialogVisible.value = true
}

const manageAgent = (agent) => {
  router.push({ name: 'for_store_agent_detail', params: { id: agent.id } })
}

const resetAgent = async (agent) => {
  try {
    await ElMessageBox.confirm(
      `Reset agent "${agent.name}"? All services within this agent will be removed.`,
      'Confirm Reset',
      { type: 'warning' }
    )
    const res = await agentsStore.resetAgentConfig(agent.id)
    if (res.success) ElMessage.success('Agent reset')
    else ElMessage.error(res.error || 'Reset failed')
  } catch (e) { /* cancelled */ }
}

const formatTime = (t) => t ? dayjs(t).format('MMM D, HH:mm') : 'Never'

const filteredAgents = computed(() => {
  return agentsStore.searchAgents(searchQuery.value)
})

const openServiceDialog = (agentId = '') => {
  dialogAgentId.value = agentId
  serviceDialogVisible.value = true
}

const handleServiceAdded = async () => {
  serviceDialogVisible.value = false
  await refreshAgents()
}

onMounted(() => {
  refreshAgents()
})
</script>

<style lang="scss" scoped>
.agent-list-container {
  width: 100%;
}

// Header
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  margin-bottom: 32px;
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

.header-actions {
  display: flex;
  gap: 12px;
  align-items: center;
}

.search-input {
  width: 240px;
}

.create-btn {
  font-weight: 500;
  border-radius: 6px;
}

.refresh-btn {
  border-color: var(--border-color);
  color: var(--text-secondary);
  &:hover { color: var(--text-primary); border-color: var(--text-secondary); background: transparent; }
}

// Grid
.agent-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  gap: 20px;
}

// Card
.agent-card {
  background: var(--bg-surface);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: 20px;
  cursor: pointer;
  transition: border-color 0.2s;
  display: flex;
  flex-direction: column;
  
  &:hover { border-color: var(--text-secondary); }
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 16px;
  
  .header-main {
    .agent-name {
      font-size: 14px;
      font-weight: 600;
      color: var(--text-primary);
      margin-bottom: 4px;
    }
    .agent-id {
      font-size: 11px;
      font-family: var(--font-mono);
      color: var(--text-placeholder);
    }
  }
  
  .status-indicator {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--border-color);
    
    &.active { background: var(--color-success); box-shadow: 0 0 0 2px rgba(16, 185, 129, 0.1); }
    &.warn { background: var(--color-warning); }
    &.error { background: var(--color-danger); }
  }
}

.card-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  
  .description {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.5;
    margin-bottom: 20px;
    flex: 1;
  }
  
  .stats-row {
    display: flex;
    justify-content: space-between;
    margin-bottom: 16px;
    padding: 12px 0;
    border-top: 1px solid var(--bg-hover);
    border-bottom: 1px solid var(--bg-hover);
    
    .stat {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 4px;
      
      .val { font-size: 14px; font-weight: 600; color: var(--text-primary); }
      .lbl { font-size: 10px; color: var(--text-secondary); text-transform: uppercase; }
      
      .val.success { color: var(--color-success); }
      .val.error { color: var(--color-danger); }
    }
  }
  
  .last-active {
    font-size: 11px;
    color: var(--text-placeholder);
    text-align: right;
    margin-bottom: 16px;
  }
}

.card-footer {
  display: flex;
  justify-content: space-between;
  padding-top: 16px;
  
  .text-btn {
    background: none;
    border: none;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
    cursor: pointer;
    padding: 0;
    
    &:hover { color: var(--text-primary); text-decoration: underline; }
    &.danger:hover { color: var(--color-danger); }
  }
}

.empty-state {
  text-align: center;
  padding: 60px 20px;
  
  .empty-icon { font-size: 48px; color: var(--text-placeholder); margin-bottom: 16px; }
  h3 { font-size: 16px; margin-bottom: 8px; color: var(--text-primary); }
  p { font-size: 13px; color: var(--text-secondary); margin-bottom: 24px; }
}
</style>
