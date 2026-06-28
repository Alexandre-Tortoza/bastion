<script setup lang="ts">
import type { Edge, Node } from '@vue-flow/core'
import type { WikiGraph } from '~/types/bastion'
import * as dagre from 'dagre'
import { MarkerType, Position, VueFlow } from '@vue-flow/core'
import { Background } from '@vue-flow/background'
import { Controls } from '@vue-flow/controls'
import { MiniMap } from '@vue-flow/minimap'

import '@vue-flow/core/dist/style.css'
import '@vue-flow/core/dist/theme-default.css'
import '@vue-flow/controls/dist/style.css'
import '@vue-flow/minimap/dist/style.css'

const nodeWidth = 240
const nodeHeight = 78

const selectedKind = ref('all')
const selectedTier = ref('all')
const isFullscreen = ref(false)

const { data, pending, error, refresh } = await useAsyncData<WikiGraph>('wiki-graph', () =>
  $fetch('/api/wiki/graph')
)

const pages = computed(() => data.value?.pages ?? [])
const links = computed(() => data.value?.links ?? [])

const kinds = computed(() => [...new Set(pages.value.map(page => page.kind).filter(Boolean))].sort())
const tiers = computed(() => [...new Set(pages.value.map(page => page.tier).filter(Boolean))].sort())

const filteredPages = computed(() => pages.value.filter((page) => {
  const kindMatches = selectedKind.value === 'all' || page.kind === selectedKind.value
  const tierMatches = selectedTier.value === 'all' || page.tier === selectedTier.value
  return kindMatches && tierMatches
}))

const pageIds = computed(() => new Set(filteredPages.value.map(page => normalisePath(page.path))))

const filteredLinks = computed(() => links.value.filter((link) => {
  const source = normalisePath(link.source)
  const target = normalisePath(link.target)
  return pageIds.value.has(source) && pageIds.value.has(target)
}))

const nodes = computed<Node[]>(() => {
  const baseNodes = filteredPages.value.map((page) => {
    const id = normalisePath(page.path)
    return {
      id,
      type: 'wiki',
      position: { x: 0, y: 0 },
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
      data: {
        path: id,
        title: page.title || id,
        kind: page.kind ?? 'other',
        tier: page.tier ?? 'unknown',
        pinned: page.pinned,
        color: kindColor(page.kind)
      }
    }
  })

  return layoutNodes(baseNodes, edges.value)
})

const edges = computed<Edge[]>(() => filteredLinks.value.map((link, index) => ({
  id: `${normalisePath(link.source)}-${normalisePath(link.target)}-${index}`,
  source: normalisePath(link.source),
  target: normalisePath(link.target),
  label: link.label || (link.anchor ? `#${link.anchor}` : undefined),
  type: 'smoothstep',
  markerEnd: { type: MarkerType.ArrowClosed, color: '#64748b' },
  style: { stroke: '#64748b', strokeWidth: 1.5 },
  labelStyle: { fill: '#cbd5e1', fontSize: 11 },
  labelBgStyle: { fill: '#0f172a', fillOpacity: 0.85 }
})))

const graphStats = computed(() => ({
  pages: filteredPages.value.length,
  links: filteredLinks.value.length,
  hiddenLinks: links.value.length - filteredLinks.value.length
}))

const kindLabels: Record<string, string> = {
  'paper': 'Paper',
  'concept': 'Conceito',
  'method': 'Método',
  'result': 'Resultado',
  'strategy': 'Estratégia',
  'decision': 'Decisão',
  'comparison': 'Comparação',
  'synthesis': 'Síntese',
  'review': 'Revisão',
  'consolidation-proposal': 'Consolidação',
  'other': 'Outro'
}

const tierLabels: Record<string, string> = {
  semantic: 'Semântica',
  episodic: 'Episódica',
  working: 'Em progresso',
  unknown: 'Sem tier'
}

function normalisePath(path: string) {
  return path.replace(/\.md$/, '')
}

function kindColor(kind?: string) {
  const colors: Record<string, string> = {
    'paper': '#38bdf8',
    'concept': '#34d399',
    'method': '#a78bfa',
    'result': '#fb7185',
    'strategy': '#fbbf24',
    'decision': '#f59e0b',
    'comparison': '#22d3ee',
    'synthesis': '#f472b6',
    'review': '#94a3b8',
    'consolidation-proposal': '#fb923c'
  }
  return colors[kind ?? ''] ?? '#64748b'
}

function tierClass(tier?: string) {
  const classes: Record<string, string> = {
    semantic: 'border-white/15 bg-slate-950/95 shadow-lg shadow-black/20',
    episodic: 'border-white/10 bg-slate-900/95',
    working: 'border-dashed border-white/20 bg-slate-950/80'
  }
  return classes[tier ?? ''] ?? 'border-white/10 bg-slate-900/95'
}

function layoutNodes(baseNodes: Node[], baseEdges: Edge[]) {
  if (baseNodes.length === 0) return []

  const graph = new dagre.graphlib.Graph()
  graph.setDefaultEdgeLabel(() => ({}))
  graph.setGraph({ rankdir: 'LR', nodesep: 52, ranksep: 116, marginx: 24, marginy: 24 })

  for (const node of baseNodes) {
    graph.setNode(node.id, { width: nodeWidth, height: nodeHeight })
  }

  for (const edge of baseEdges) {
    graph.setEdge(edge.source, edge.target)
  }

  dagre.layout(graph)

  return baseNodes.map((node) => {
    const position = graph.node(node.id)
    return {
      ...node,
      position: {
        x: position.x - nodeWidth / 2,
        y: position.y - nodeHeight / 2
      }
    }
  })
}

function minimapColor(node: Node) {
  return String(node.data?.color ?? '#64748b')
}

function onNodeClick(payload: { node: Node }) {
  const path = String(payload.node.data?.path ?? payload.node.id)
  navigateTo(`/wiki/${path}`)
}

function graphCardElement() {
  return document.querySelector<HTMLElement>('.wiki-graph-card')
}

async function syncGraphViewport() {
  await nextTick()
  window.dispatchEvent(new Event('resize'))
}

async function toggleFullscreen() {
  const element = graphCardElement()

  if (!document.fullscreenElement && element?.requestFullscreen) {
    await element.requestFullscreen()
    isFullscreen.value = true
  } else if (document.fullscreenElement && document.exitFullscreen) {
    await document.exitFullscreen()
    isFullscreen.value = false
  } else {
    isFullscreen.value = !isFullscreen.value
  }

  await syncGraphViewport()
}

function onFullscreenChange() {
  isFullscreen.value = document.fullscreenElement === graphCardElement()
  void syncGraphViewport()
}

onMounted(() => {
  document.addEventListener('fullscreenchange', onFullscreenChange)
})

onBeforeUnmount(() => {
  document.removeEventListener('fullscreenchange', onFullscreenChange)
})
</script>

<template>
  <AppCard
    class="wiki-graph-card mb-6 overflow-hidden transition-all"
    :class="isFullscreen ? 'fixed inset-0 z-50 m-0 rounded-none' : ''"
  >
    <div class="flex flex-col gap-4 border-b border-default p-4 lg:flex-row lg:items-center lg:justify-between">
      <div>
        <p class="text-sm font-medium text-primary-300">
          Mapa de conhecimento
        </p>
        <h2 class="text-lg font-semibold">
          Ligações entre notas
        </h2>
        <p class="mt-1 text-sm text-text/60">
          Visualize wikilinks como conexões navegáveis entre papers, conceitos, métodos e decisões.
        </p>
      </div>

      <div class="flex flex-wrap items-end gap-3">
        <label class="grid gap-1 text-xs text-text/60">
          Tipo
          <select
            v-model="selectedKind"
            class="h-9 rounded-md border border-default bg-bg px-3 text-sm text-text outline-none focus:border-primary-500"
          >
            <option value="all">
              Todos
            </option>
            <option
              v-for="kind in kinds"
              :key="kind"
              :value="kind"
            >
              {{ kindLabels[kind] ?? kind }}
            </option>
          </select>
        </label>

        <label class="grid gap-1 text-xs text-text/60">
          Memória
          <select
            v-model="selectedTier"
            class="h-9 rounded-md border border-default bg-bg px-3 text-sm text-text outline-none focus:border-primary-500"
          >
            <option value="all">
              Todas
            </option>
            <option
              v-for="tier in tiers"
              :key="tier"
              :value="tier"
            >
              {{ tierLabels[tier] ?? tier }}
            </option>
          </select>
        </label>

        <UButton
          icon="i-lucide-refresh-cw"
          color="neutral"
          variant="subtle"
          :loading="pending"
          @click="refresh()"
        >
          Atualizar
        </UButton>

        <UButton
          :icon="isFullscreen ? 'i-lucide-minimize-2' : 'i-lucide-maximize-2'"
          color="neutral"
          variant="subtle"
          @click="toggleFullscreen"
        >
          {{ isFullscreen ? 'Sair' : 'Fullscreen' }}
        </UButton>
      </div>
    </div>

    <div class="grid grid-cols-2 gap-px border-b border-default bg-default/60 text-sm md:grid-cols-4">
      <div class="bg-bg px-4 py-3">
        <p class="text-xs text-text/50">
          Notas
        </p>
        <p class="font-semibold">
          {{ graphStats.pages }}
        </p>
      </div>
      <div class="bg-bg px-4 py-3">
        <p class="text-xs text-text/50">
          Conexões visíveis
        </p>
        <p class="font-semibold">
          {{ graphStats.links }}
        </p>
      </div>
      <div class="bg-bg px-4 py-3">
        <p class="text-xs text-text/50">
          Conexões filtradas
        </p>
        <p class="font-semibold">
          {{ Math.max(graphStats.hiddenLinks, 0) }}
        </p>
      </div>
      <div class="bg-bg px-4 py-3">
        <p class="text-xs text-text/50">
          Layout
        </p>
        <p class="font-semibold">
          dagre LR
        </p>
      </div>
    </div>

    <div
      v-if="pending"
      class="grid min-h-[560px] place-items-center bg-slate-950/40 p-8"
    >
      <div class="w-full max-w-xl space-y-3">
        <AppSkeleton class="h-8 w-52" />
        <AppSkeleton class="h-32 w-full" />
        <AppSkeleton class="h-32 w-5/6" />
      </div>
    </div>

    <div
      v-else-if="error"
      class="grid min-h-[420px] place-items-center bg-slate-950/40 p-8 text-center"
    >
      <div>
        <UIcon
          name="i-lucide-triangle-alert"
          class="mx-auto mb-3 size-10 text-warning-400"
        />
        <h3 class="font-semibold">
          Não foi possível carregar o grafo
        </h3>
        <p class="mt-2 text-sm text-text/60">
          Verifique se o backend está rodando com o endpoint /api/wiki/graph.
        </p>
      </div>
    </div>

    <div
      v-else-if="nodes.length === 0"
      class="grid min-h-[420px] place-items-center bg-slate-950/40 p-8 text-center"
    >
      <div>
        <UIcon
          name="i-lucide-network"
          class="mx-auto mb-3 size-10 text-primary-300"
        />
        <h3 class="font-semibold">
          Nenhuma nota para mostrar
        </h3>
        <p class="mt-2 text-sm text-text/60">
          Ingira papers ou ajuste os filtros para visualizar conexões.
        </p>
      </div>
    </div>

    <ClientOnly v-else>
      <div
        class="bg-slate-950/60"
        :class="isFullscreen ? 'h-[calc(100vh-220px)]' : 'h-[620px]'"
      >
        <VueFlow
          :nodes="nodes"
          :edges="edges"
          :nodes-draggable="true"
          :fit-view-on-init="true"
          class="wiki-graph"
          @node-click="onNodeClick"
        >
          <template #node-wiki="{ data: nodeData }">
            <div
              :class="[
                'w-[240px] rounded-xl border px-3 py-2 text-left transition-transform hover:scale-[1.02]',
                tierClass(nodeData.tier)
              ]"
              :style="{ boxShadow: `0 0 0 1px ${nodeData.color}33, 0 14px 40px rgb(0 0 0 / 0.22)` }"
            >
              <div class="mb-2 flex items-center gap-2">
                <span
                  class="size-2.5 rounded-full"
                  :style="{ backgroundColor: nodeData.color }"
                />
                <span class="truncate text-[11px] uppercase tracking-wide text-text/50">
                  {{ kindLabels[nodeData.kind] ?? nodeData.kind }}
                </span>
                <UIcon
                  v-if="nodeData.pinned"
                  name="i-lucide-pin"
                  class="ml-auto size-3 text-warning-300"
                />
              </div>
              <p class="line-clamp-2 text-sm font-semibold leading-snug text-text">
                {{ nodeData.title }}
              </p>
              <p class="mt-1 truncate text-[11px] text-text/45">
                {{ nodeData.path }}
              </p>
            </div>
          </template>

          <Background
            pattern-color="#334155"
            :gap="18"
          />
          <Controls />
          <MiniMap
            pannable
            zoomable
            :node-color="minimapColor"
            mask-color="rgb(2 6 23 / 0.74)"
          />
        </VueFlow>
      </div>
    </ClientOnly>
  </AppCard>
</template>

<style scoped>
.wiki-graph :deep(.vue-flow__node) {
  background: transparent;
  border: 0;
  color: inherit;
  padding: 0;
}

.wiki-graph :deep(.vue-flow__edge-textbg) {
  stroke: transparent;
}

.wiki-graph :deep(.vue-flow__controls) {
  border: 1px solid rgb(51 65 85 / 0.75);
  box-shadow: none;
}

.wiki-graph :deep(.vue-flow__controls-button) {
  background: rgb(15 23 42 / 0.94);
  border-bottom: 1px solid rgb(51 65 85 / 0.75);
  color: rgb(226 232 240);
}

.wiki-graph :deep(.vue-flow__controls-button svg) {
  color: rgb(226 232 240);
  fill: none;
  stroke: currentColor;
}

.wiki-graph :deep(.vue-flow__controls-button:hover) {
  background: rgb(30 41 59 / 0.96);
}

.wiki-graph :deep(.vue-flow__minimap) {
  background: rgb(15 23 42 / 0.94);
  border: 1px solid rgb(51 65 85 / 0.75);
  border-radius: 12px;
}
</style>
