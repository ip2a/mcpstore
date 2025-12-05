<template>
  <div class="external-embed">
    <iframe :src="url" referrerpolicy="no-referrer" />
  </div>
  
</template>

<script setup>
import { computed } from 'vue'
import { useRoute } from 'vue-router'

const route = useRoute()
const url = computed(() => route.meta?.url)
</script>

<style scoped>
.external-embed {
  width: 100%;
  height: calc(100vh - 60px);
}
.external-embed iframe {
  width: 100%;
  height: 100%;
  border: 0;
}
</style>


