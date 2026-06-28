<script setup lang="ts">
import type { Reference } from '~/types/bastion'

defineProps<{
  references: Reference[]
}>()

const emit = defineEmits<{
  'open-pdf': [url: string, page: number]
  'open-wiki': [path: string]
}>()
</script>

<template>
  <div class="h-full overflow-y-auto p-3 space-y-3">
    <h3 class="text-sm font-semibold text-gray-400 uppercase tracking-wider">
      Referências
    </h3>

    <div v-if="references.length === 0" class="text-center text-gray-600 mt-12 text-sm">
      Nenhuma referência ainda — faça uma pergunta no chat.
    </div>

    <UCard
      v-for="(ref, i) in references"
      :key="i"
      class="text-sm"
    >
      <div class="font-medium text-gray-200 mb-1">
        {{ ref.title }}
      </div>
      <!-- eslint-disable-next-line vue/no-v-html -->
      <div class="text-gray-400 text-xs line-clamp-3" v-html="ref.excerpt" />

      <div class="mt-2 flex gap-2">
        <UButton
          v-if="ref.kind === 'paper' && ref.pdf_url"
          size="xs"
          variant="outline"
          icon="i-heroicons-document"
          @click="emit('open-pdf', ref.pdf_url!, ref.page ?? 1)"
        >
          Ver p. {{ ref.page }}
        </UButton>
        <UButton
          v-if="ref.wiki_path"
          size="xs"
          variant="ghost"
          icon="i-heroicons-book-open"
          @click="emit('open-wiki', ref.wiki_path!)"
        >
          Wiki
        </UButton>
      </div>
    </UCard>
  </div>
</template>
