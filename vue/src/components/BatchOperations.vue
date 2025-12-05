<template>
  <div class="batch-operations">
    <!-- Selection Header -->
    <div v-if="showHeader" class="selection-header">
      <div class="selection-info">
        <el-checkbox
          v-model="allSelected"
          :indeterminate="isIndeterminate"
          @change="handleSelectAll"
        >
          已选择 {{ selectedItems.length }} / {{ totalItems }} 项
        </el-checkbox>
      </div>
      <div class="batch-actions" v-if="selectedItems.length > 0">
        <slot name="batch-actions" :selectedItems="selectedItems">
          <!-- Default batch actions -->
          <el-button size="small" @click="$emit('batch-edit', selectedItems)">
            批量编辑
          </el-button>
          <el-button size="small" type="danger" @click="$emit('batch-delete', selectedItems)">
            批量删除
          </el-button>
        </slot>
      </div>
    </div>

    <!-- Selection Overlay -->
    <div v-if="showOverlay && selectedItems.length > 0" class="selection-overlay">
      <div class="overlay-content">
        <span class="selection-count">{{ selectedItems.length }} 项已选中</span>
        <div class="overlay-actions">
          <slot name="overlay-actions" :selectedItems="selectedItems">
            <el-button size="small" @click="$emit('batch-edit', selectedItems)">
              编辑
            </el-button>
            <el-button size="small" type="danger" @click="$emit('batch-delete', selectedItems)">
              删除
            </el-button>
          </slot>
          <el-button size="small" @click="clearSelection">清除选择</el-button>
        </div>
      </div>
    </div>

    <!-- Batch Edit Dialog -->
    <el-dialog
      v-model="showBatchEditDialog"
      title="批量编辑"
      width="600px"
      @close="resetBatchEditForm"
    >
      <el-form :model="batchEditForm" label-width="100px">
        <el-form-item label="编辑字段">
          <el-select
            v-model="batchEditForm.field"
            placeholder="请选择要编辑的字段"
            @change="onFieldChange"
          >
            <el-option
              v-for="field in editableFields"
              :key="field.value"
              :label="field.label"
              :value="field.value"
            />
          </el-select>
        </el-form-item>
        
        <el-form-item
          v-if="batchEditForm.field"
          :label="getFieldLabel(batchEditForm.field)"
        >
          <!-- Text Input -->
          <el-input
            v-if="getFieldType(batchEditForm.field) === 'text'"
            v-model="batchEditForm.value"
            :placeholder="`请输入${getFieldLabel(batchEditForm.field)}`"
          />
          
          <!-- Number Input -->
          <el-input-number
            v-else-if="getFieldType(batchEditForm.field) === 'number'"
            v-model="batchEditForm.value"
            :placeholder="`请输入${getFieldLabel(batchEditForm.field)}`"
          />
          
          <!-- Select -->
          <el-select
            v-else-if="getFieldType(batchEditForm.field) === 'select'"
            v-model="batchEditForm.value"
            :placeholder="`请选择${getFieldLabel(batchEditForm.field)}`"
          >
            <el-option
              v-for="option in getFieldOptions(batchEditForm.field)"
              :key="option.value"
              :label="option.label"
              :value="option.value"
            />
          </el-select>
          
          <!-- Switch -->
          <el-switch
            v-else-if="getFieldType(batchEditForm.field) === 'switch'"
            v-model="batchEditForm.value"
          />
        </el-form-item>
      </el-form>
      
      <template #footer>
        <el-button @click="showBatchEditDialog = false">取消</el-button>
        <el-button
          type="primary"
          @click="confirmBatchEdit"
          :loading="isBatchEditing"
        >
          确定
        </el-button>
      </template>
    </el-dialog>

    <!-- Batch Delete Confirm -->
    <el-dialog
      v-model="showBatchDeleteDialog"
      title="批量删除确认"
      width="500px"
    >
      <div class="batch-delete-confirm">
        <el-icon class="warning-icon"><WarningFilled /></el-icon>
        <p>确定要删除选中的 {{ selectedItems.length }} 项吗？</p>
        <p class="warning-text">此操作不可撤销，请谨慎操作。</p>
        <div class="delete-preview">
          <div
            v-for="item in selectedItems.slice(0, 5)"
            :key="getItemKey(item)"
            class="preview-item"
          >
            {{ getItemName(item) }}
          </div>
          <div v-if="selectedItems.length > 5" class="preview-more">
            ... 还有 {{ selectedItems.length - 5 }} 项
          </div>
        </div>
      </div>
      
      <template #footer>
        <el-button @click="showBatchDeleteDialog = false">取消</el-button>
        <el-button
          type="danger"
          @click="confirmBatchDelete"
          :loading="isBatchDeleting"
        >
          删除
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { WarningFilled } from '@element-plus/icons-vue'

// Props
const props = defineProps({
  items: {
    type: Array,
    default: () => []
  },
  itemKey: {
    type: String,
    default: 'id'
  },
  itemName: {
    type: String,
    default: 'name'
  },
  showHeader: {
    type: Boolean,
    default: true
  },
  showOverlay: {
    type: Boolean,
    default: false
  },
  editableFields: {
    type: Array,
    default: () => []
  }
})

// Emits
const emit = defineEmits([
  'selection-change',
  'select-all',
  'batch-edit',
  'batch-delete',
  'update:selectedItems'
])

// Reactive Data
const selectedItems = ref([])
const showBatchEditDialog = ref(false)
const showBatchDeleteDialog = ref(false)
const isBatchEditing = ref(false)
const isBatchDeleting = ref(false)

const batchEditForm = ref({
  field: '',
  value: null
})

// Computed Properties
const totalItems = computed(() => props.items.length)

const allSelected = computed({
  get: () => selectedItems.value.length === totalItems.value && totalItems.value > 0,
  set: (value) => handleSelectAll(value)
})

const isIndeterminate = computed(() => {
  return selectedItems.value.length > 0 && selectedItems.value.length < totalItems.value
})

// Methods
const handleSelectAll = (checked) => {
  if (checked) {
    selectedItems.value = [...props.items]
  } else {
    selectedItems.value = []
  }
  emit('select-all', checked)
  emit('selection-change', selectedItems.value)
}

const toggleSelection = (item) => {
  const index = selectedItems.value.findIndex(
    selected => selected[props.itemKey] === item[props.itemKey]
  )
  
  if (index === -1) {
    selectedItems.value.push(item)
  } else {
    selectedItems.value.splice(index, 1)
  }
  
  emit('selection-change', selectedItems.value)
}

const isSelected = (item) => {
  return selectedItems.value.some(
    selected => selected[props.itemKey] === item[props.itemKey]
  )
}

const clearSelection = () => {
  selectedItems.value = []
  emit('selection-change', selectedItems.value)
}

const getItemKey = (item) => {
  return item[props.itemKey]
}

const getItemName = (item) => {
  return item[props.itemName]
}

const getFieldLabel = (field) => {
  const fieldConfig = props.editableFields.find(f => f.value === field)
  return fieldConfig?.label || field
}

const getFieldType = (field) => {
  const fieldConfig = props.editableFields.find(f => f.value === field)
  return fieldConfig?.type || 'text'
}

const getFieldOptions = (field) => {
  const fieldConfig = props.editableFields.find(f => f.value === field)
  return fieldConfig?.options || []
}

const onFieldChange = () => {
  // Reset value when field changes
  batchEditForm.value.value = null
}

const resetBatchEditForm = () => {
  batchEditForm.value = {
    field: '',
    value: null
  }
}

const confirmBatchEdit = async () => {
  if (!batchEditForm.value.field || batchEditForm.value.value === null) {
    ElMessage.warning('请填写完整的编辑信息')
    return
  }
  
  isBatchEditing.value = true
  try {
    await emit('batch-edit', {
      items: selectedItems.value,
      field: batchEditForm.value.field,
      value: batchEditForm.value.value
    })
    
    ElMessage.success('批量编辑成功')
    showBatchEditDialog.value = false
    clearSelection()
  } catch (error) {
    ElMessage.error('批量编辑失败')
    console.error('Batch edit failed:', error)
  } finally {
    isBatchEditing.value = false
  }
}

const confirmBatchDelete = async () => {
  isBatchDeleting.value = true
  try {
    await emit('batch-delete', selectedItems.value)
    
    ElMessage.success('批量删除成功')
    showBatchDeleteDialog.value = false
    clearSelection()
  } catch (error) {
    ElMessage.error('批量删除失败')
    console.error('Batch delete failed:', error)
  } finally {
    isBatchDeleting.value = false
  }
}

// Watch for external selection changes
watch(() => props.items, () => {
  // Filter out selected items that are no longer in the list
  selectedItems.value = selectedItems.value.filter(selected =>
    props.items.some(item => item[props.itemKey] === selected[props.itemKey])
  )
}, { deep: true })

// Expose methods
defineExpose({
  toggleSelection,
  isSelected,
  clearSelection,
  selectedItems,
  showBatchEditDialog: () => showBatchEditDialog.value = true,
  showBatchDeleteDialog: () => showBatchDeleteDialog.value = true
})
</script>

<style scoped>
.batch-operations {
  width: 100%;
}

/* Selection Header */
.selection-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  background: var(--bg-color-secondary);
  border-radius: var(--border-radius-md);
  margin-bottom: 16px;
}

.selection-info {
  display: flex;
  align-items: center;
}

.batch-actions {
  display: flex;
  gap: 8px;
}

/* Selection Overlay */
.selection-overlay {
  position: fixed;
  bottom: 20px;
  left: 50%;
  transform: translateX(-50%);
  background: var(--bg-color);
  border-radius: var(--border-radius-lg);
  box-shadow: var(--shadow-lg);
  padding: 12px 20px;
  z-index: 1000;
  display: flex;
  align-items: center;
  gap: 16px;
  border: 1px solid var(--border-lighter);
}

.overlay-content {
  display: flex;
  align-items: center;
  gap: 16px;
}

.selection-count {
  font-weight: var(--font-weight-medium);
  color: var(--text-primary);
}

.overlay-actions {
  display: flex;
  gap: 8px;
}

/* Dialog Styles */
.batch-delete-confirm {
  text-align: center;
  padding: 20px 0;
}

.warning-icon {
  font-size: 48px;
  color: var(--warning-color);
  margin-bottom: 16px;
}

.batch-delete-confirm p {
  margin: 0 0 8px 0;
}

.warning-text {
  color: var(--text-secondary) !important;
  font-size: var(--font-size-sm) !important;
}

.delete-preview {
  margin-top: 16px;
  padding: 12px;
  background: var(--bg-color-secondary);
  border-radius: var(--border-radius-md);
  text-align: left;
}

.preview-item {
  padding: 4px 0;
  color: var(--text-regular);
  font-size: var(--font-size-sm);
}

.preview-more {
  padding: 4px 0;
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
  font-style: italic;
}

/* Responsive Design */
@media (max-width: 768px) {
  .selection-header {
    flex-direction: column;
    gap: 12px;
    align-items: flex-start;
  }
  
  .selection-overlay {
    width: 90%;
    max-width: 400px;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
  }
  
  .overlay-content {
    flex-direction: column;
    width: 100%;
  }
  
  .overlay-actions {
    width: 100%;
    justify-content: space-between;
  }
}
</style>