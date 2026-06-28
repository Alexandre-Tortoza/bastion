<script setup lang="ts">
import type { WikiPage } from '~/types/bastion'

const props = defineProps<{
  pages: WikiPage[]
  selectedPath?: string
}>()

const emit = defineEmits<{
  select: [page: WikiPage]
}>()

const collapsed = ref<Record<string, boolean>>({})

const groups = computed(() => {
  const g: Record<string, WikiPage[]> = {}
  for (const p of props.pages) {
    const dir = p.path.includes('/') ? p.path.split('/')[0]! : '_root'
    if (!g[dir]) g[dir] = []
    g[dir]!.push(p)
  }
  return g
})

const dirLabels: Record<string, string> = {
  papers: 'Papers',
  concepts: 'Conceitos',
  methods: 'Métodos',
  decisions: 'Decisões',
  comparisons: 'Comparações',
  synthesis: 'Sínteses',
  reviews: 'Revisões',
  _root: 'Raiz',
}

const tierColor: Record<string, string> = {
  semantic: 'bg-green-500',
  episodic: 'bg-yellow-500',
  working: 'bg-gray-500',
}

const statusColor: Record<string, string> = {
  proposed: 'blue',
  accepted: 'green',
  superseded: 'gray',
  rejected: 'red',
  ingested: 'indigo',
  reviewed: 'green',
}

function toggle(dir: string) {
  collapsed.value[dir] = !collapsed.value[dir]
}
</script>

<template>
  <nav class="h-full overflow-y-auto text-sm p-2 space-y-1">
    <div v-for="(items, dir) in groups" :key="dir">
      <!-- group header -->
      <button
        class="w-full flex items-center justify-between px-2 py-1 rounded hover:bg-gray-800 text-gray-400 font-semibold uppercase tracking-wider text-xs"
        @click="toggle(String(dir))"
      >
        {{ dirLabels[String(dir)] ?? dir }}
        <UIcon
          :name="collapsed[String(dir)] ? 'i-heroicons-chevron-right' : 'i-heroicons-chevron-down'"
          class="w-3 h-3"
        />
      </button>

      <!-- items -->
      <div v-show="!collapsed[String(dir)]" class="ml-2 space-y-0.5">
        <button
          v-for="page in items"
          :key="page.path"
          :class="[
            'w-full flex items-center gap-2 px-2 py-1 rounded text-left text-gray-300 hover:bg-gray-800 hover:text-white transition-colors',
            selectedPath === page.path ? 'bg-gray-800 text-white' : '',
          ]"
          @click="emit('select', page)"
        >
          <!-- tier dot -->
          <span
            :class="['inline-block w-2 h-2 rounded-full flex-shrink-0', tierColor[page.tier ?? ''] ?? 'bg-gray-600']"
          />
          <span class="truncate flex-1">{{ page.title || page.path }}</span>
          <UBadge
            v-if="page.status && page.kind === 'decision'"
            :color="(statusColor[page.status] as any) ?? 'gray'"
            size="xs"
            variant="subtle"
          >
            {{ page.status }}
          </UBadge>
        </button>
      </div>
    </div>
  </nav>
</template>
