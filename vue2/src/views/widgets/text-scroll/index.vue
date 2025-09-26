<template>
  <div class="page-content">
    <!-- 基础用法 -->
    <ArtTextScroll
      text="MCPStore 是一个全面的模型上下文协议（MCP）管理系统 <a target='_blank' href='https://doc.mcpstore.wiki/'>点击我 </a>访问官方文档"
      showClose
    />

    <!-- 使用不同的类型 -->
    <ArtTextScroll type="success" text="这是一条成功类型的滚动公告" showClose />

    <ArtTextScroll type="warning" text="这是一条警告类型的滚动公告" showClose />

    <ArtTextScroll type="danger" text="这是一条危险类型的滚动公告" showClose />

    <ArtTextScroll type="info" text="这是一条信息类型的滚动公告" showClose />

    <!-- 可关闭 -->
    <ArtTextScroll text="这是一条可关闭的滚动公告" showClose @close="handleClose" />

    <!-- 使用打字机效果 -->
    <ArtTextScroll
      text="这是一条打字机效果的滚动公告，打字机速度为 200 毫秒"
      typewriter
      :typewriter-speed="200"
    />

    <!-- 自定义速度和方向 -->
    <ArtTextScroll text="这是一条速度较慢、向右滚动的公告" :speed="30" direction="right" />
  </div>
</template>

<script setup lang="ts">
  const handleClose = () => {
    console.log('关闭')
  }
</script>

<style lang="scss" scoped>
  .page-content {
    :deep(.text-scroll-container) {
      margin-bottom: 20px;
    }
  }
</style>
