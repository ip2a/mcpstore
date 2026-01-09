<template>
  <div class="not-found">
    <div class="not-found-content">
      <div class="error-code">
        404
      </div>
      <div class="error-title">
        页面未找到
      </div>
      <div class="error-description">
        抱歉，您访问的页面不存在或已被移除
      </div>
      <div class="error-actions">
        <el-button
          type="primary"
          @click="goHome"
        >
          返回首页
        </el-button>
        <el-button @click="goBack">
          返回上页
        </el-button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { useRouter } from 'vue-router'

const router = useRouter()

const goHome = () => {
  router.push('/')
}

const goBack = () => {
  router.back()
}
</script>

<style lang="scss" scoped>
.not-found {
  height: 100vh;
  @include flex-center;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  
  .not-found-content {
    text-align: center;
    color: white;
    
    .error-code {
      font-size: 120px;
      font-weight: bold;
      margin-bottom: 20px;
      opacity: 0.8;
    }
    
    .error-title {
      font-size: 32px;
      font-weight: 500;
      margin-bottom: 16px;
    }
    
    .error-description {
      font-size: 16px;
      margin-bottom: 32px;
      opacity: 0.9;
    }
    
    .error-actions {
      display: flex;
      gap: 16px;
      justify-content: center;
    }
  }
}
</style>
