<script setup lang="ts">
import type { WikiPage } from '~/types/bastion'

const { data: papersData, pending: papersPending } = await useAsyncData('recent-papers', () =>
  $fetch<{ pages: WikiPage[] }>('/api/wiki/pages', { query: { kind: 'paper', limit: 5 } }),
)
const { data: decisionsData, pending: decisionsPending } = await useAsyncData('open-decisions', () =>
  $fetch<{ decisions: unknown[] }>('/api/wiki/decisions', { query: { status: 'proposed' } }),
)
const { data: logData, pending: logPending } = await useAsyncData('log', () =>
  $fetch<{ log: Array<{ heading: string }>; raw: string }>('/api/wiki/log', { query: { limit: 5 } }),
)

const papers = computed(() => papersData.value?.pages ?? [])
const openDecisions = computed(() => decisionsData.value?.decisions ?? [])
const logHeadings = computed(() =>
  (logData.value?.log ?? []).map((e: { heading: string }) => e.heading),
)
</script>

<template>
  <AppContainer class="h-full overflow-y-auto py-8">
    <h1 class="text-2xl font-bold mb-6">
      Dashboard
    </h1>

    <div class="grid grid-cols-12 gap-6">
      <!-- Papers recentes -->
      <div class="col-span-12 lg:col-span-8 space-y-6">
        <AppCard>
          <template #header>
            <div class="flex items-center justify-between">
              <span class="font-semibold">Papers Recentes</span>
              <AppButton to="/wiki" variant="ghost" size="xs">
                Ver todos
              </AppButton>
            </div>
          </template>

          <div v-if="papersPending" class="space-y-3 py-2">
            <AppSkeleton v-for="i in 4" :key="i" class="h-5 w-full" />
          </div>

          <div v-else-if="papers.length === 0" class="text-center py-10 space-y-3">
            <p class="text-muted">
              A wiki ainda não tem papers.
            </p>
            <AppButton to="/ingest" icon="i-lucide-file-plus">
              Envie seu primeiro artigo
            </AppButton>
          </div>

          <ul v-else class="divide-y divide-default">
            <li
              v-for="p in papers"
              :key="p.path"
              class="py-3 flex items-center justify-between"
            >
              <NuxtLink
                :to="`/wiki/${p.path.replace(/\.md$/, '')}`"
                class="text-sm font-medium hover:text-primary-300 transition-colors"
              >
                {{ p.title || p.path }}
              </NuxtLink>
              <span class="text-xs text-muted shrink-0">{{ p.updated_at }}</span>
            </li>
          </ul>
        </AppCard>
      </div>

      <!-- Decisões + Atividade -->
      <div class="col-span-12 lg:col-span-4 space-y-4">
        <AppCard>
          <template #header>
            <span class="font-semibold">Decisões Propostas</span>
          </template>

          <div v-if="decisionsPending">
            <AppSkeleton class="h-8 w-full" />
          </div>
          <div v-else-if="openDecisions.length === 0" class="text-sm text-muted py-2">
            Nenhuma decisão pendente.
          </div>
          <div v-else class="text-sm">
            <span class="text-2xl font-bold text-primary-300">{{ openDecisions.length }}</span>
            decisão(ões) aguardando revisão.
            <NuxtLink to="/wiki/decisions" class="block mt-1 text-xs text-primary-300 hover:underline">
              Ver todas →
            </NuxtLink>
          </div>
        </AppCard>

        <AppCard>
          <template #header>
            <span class="font-semibold">Última Atividade</span>
          </template>

          <div v-if="logPending" class="space-y-2">
            <AppSkeleton v-for="i in 4" :key="i" class="h-4 w-full" />
          </div>
          <div v-else-if="logHeadings.length === 0" class="text-sm text-muted py-2">
            Nenhuma atividade registrada.
          </div>
          <ul v-else class="space-y-1">
            <li
              v-for="h in logHeadings"
              :key="h"
              class="text-xs text-muted truncate"
            >
              {{ h.replace(/^## /, '') }}
            </li>
          </ul>
        </AppCard>
      </div>
    </div>
  </AppContainer>
</template>
