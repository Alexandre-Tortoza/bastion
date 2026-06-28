<script setup lang="ts">
withDefaults(
  defineProps<{
    src?: string;
    error?: string;
    loading?: boolean;
  }>(),
  {
    src: "",
    error: "",
    loading: false,
  },
);
</script>

<template>
  <div class="h-full min-h-0 bg-viewer p-6">
    <div
      class="paper-page relative mx-auto overflow-hidden rounded-sm bg-surface text-text shadow-2xl shadow-black/60 ring-1 ring-primary-800/70"
    >
      <iframe
        v-if="src"
        :src="src"
        class="h-full w-full border-0 transition duration-300"
        :class="loading ? 'scale-[1.01] blur-sm opacity-55' : 'blur-0 opacity-100'"
        title="Preview PDF"
      />

      <div v-if="loading && !src" class="h-full p-8">
        <div class="space-y-8">
          <div class="space-y-3">
            <AppSkeleton class="h-7 w-2/3" />
            <AppSkeleton class="h-4 w-1/3" />
          </div>
          <div class="space-y-3">
            <AppSkeleton class="h-4 w-full" />
            <AppSkeleton class="h-4 w-11/12" />
            <AppSkeleton class="h-4 w-10/12" />
            <AppSkeleton class="h-4 w-full" />
            <AppSkeleton class="h-4 w-8/12" />
          </div>
          <div class="space-y-3 pt-4">
            <AppSkeleton class="h-5 w-1/4" />
            <AppSkeleton class="h-4 w-full" />
            <AppSkeleton class="h-4 w-9/12" />
            <AppSkeleton class="h-4 w-10/12" />
          </div>
        </div>
      </div>

      <div
        v-else-if="loading"
        class="absolute inset-0 flex items-center justify-center bg-surface/65 p-8 backdrop-blur-md"
      >
        <div
          class="flex w-full max-w-xs flex-col items-center rounded-2xl border border-white/10 bg-bg/65 p-6 text-center shadow-2xl shadow-black/40"
        >
          <div
            class="mb-4 size-8 animate-spin rounded-full border-2 border-primary-800/30 border-t-primary-300"
            aria-hidden="true"
          />
          <div class="text-sm font-medium text-text/80">
            Atualizando ...
          </div>
        </div>
      </div>

      <div
        v-else-if="error"
        class="absolute inset-0 overflow-auto bg-surface/95 p-6 text-sm text-red-300 whitespace-pre-wrap"
      >
        {{ error }}
      </div>

      <div v-else-if="!src" class="h-full flex items-center justify-center text-sm text-text/70">
        Preparando preview...
      </div>
    </div>
  </div>
</template>

<style scoped>
.paper-page {
  max-width: 210mm;
  height: 100%;
}
</style>
