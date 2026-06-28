<script setup lang="ts">
defineProps<{
  suggestionsRaw: string
  loading: boolean
}>()
</script>

<template>
  <div class="h-full overflow-y-auto p-3">
    <h3 class="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-3">
      Sugestões
    </h3>

    <!-- skeleton -->
    <div v-if="loading" class="space-y-3">
      <div v-for="i in 4" :key="i" class="h-14 bg-gray-800 rounded-lg animate-pulse" />
    </div>

    <!-- empty -->
    <div v-else-if="!suggestionsRaw" class="text-center text-gray-600 mt-12 text-sm">
      Clique em "Revisar" para checar seu LaTeX contra a wiki.
    </div>

    <!-- content -->
    <div
      v-else
      class="prose prose-invert prose-sm max-w-none"
      v-html="suggestionsRaw.replace(/\n/g, '<br>')"
    />
  </div>
</template>
