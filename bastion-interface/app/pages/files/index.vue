<script setup lang="ts">
import type { WikiPage } from '~/types/bastion'

definePageMeta({ ssr: false })

const { upload, status, reset } = useIngest()

const modalOpen = ref(false)
const file = ref<File | null>(null)
const dragOver = ref(false)
const uploading = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)

const steps = ['received', 'converting', 'extracting', 'integrating', 'indexed'] as const
const stepLabels: Record<string, string> = {
  received: 'Recebido',
  converting: 'Convertendo',
  extracting: 'Extraindo',
  integrating: 'Integrando',
  indexed: 'Indexado',
  embedding: 'Embeddings',
}

function currentStepIndex() {
  if (!status.value) return -1
  return steps.indexOf(status.value.step as typeof steps[number])
}

function onDrop(e: DragEvent) {
  dragOver.value = false
  const f = e.dataTransfer?.files[0]
  if (f?.type === 'application/pdf') file.value = f
}

function onFileChange(e: Event) {
  const f = (e.target as HTMLInputElement).files?.[0]
  if (f) file.value = f
}

async function send() {
  if (!file.value) return
  uploading.value = true
  try {
    await upload(file.value)
  } catch {
    // status.error is set by useIngest — UI handles display
  } finally {
    uploading.value = false
  }
}

function closeModal(done = false) {
  reset()
  file.value = null
  modalOpen.value = false
  if (done) refresh()
}

const { data, pending, refresh } = await useAsyncData('memory-files', () =>
  $fetch<{ pages: WikiPage[] }>('/api/wiki/pages')
)

const papers = computed(() => (data.value?.pages ?? []).filter(page => page.kind === 'paper'))
</script>

<template>
  <AppContainer class="h-full overflow-y-auto py-8 space-y-6">
    <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
      <div>
        <p class="text-sm font-medium text-primary-300">
          Memória
        </p>
        <h1 class="text-2xl font-bold">
          Arquivos
        </h1>
        <p class="mt-2 max-w-2xl text-sm text-text/70">
          Papers carregados na memória do sistema. Use a análise para transformar um paper em conceitos, relações e referências consultáveis.
        </p>
      </div>

      <AppButton
        icon="i-lucide-file-search"
        color="primary"
        @click="modalOpen = true"
      >
        Analisar paper
      </AppButton>
    </div>

    <AppCard>
      <div
        v-if="pending"
        class="space-y-3"
      >
        <AppSkeleton class="h-5 w-1/3" />
        <AppSkeleton class="h-5 w-2/3" />
        <AppSkeleton class="h-5 w-1/2" />
      </div>

      <div
        v-else-if="papers.length === 0"
        class="py-12 text-center"
      >
        <h2 class="text-lg font-semibold">
          Nenhum paper carregado
        </h2>
        <p class="mx-auto mt-2 max-w-md text-sm text-text/70">
          Analise o primeiro paper para começar a construir a memória e a rede de conhecimento.
        </p>
      </div>

      <ul
        v-else
        class="divide-y divide-default"
      >
        <li
          v-for="paper in papers"
          :key="paper.path"
          class="flex items-center justify-between gap-4 py-3"
        >
          <div>
            <NuxtLink
              :to="`/wiki/${paper.path.replace(/\.md$/, '')}`"
              class="font-medium hover:text-primary-300 transition-colors"
            >
              {{ paper.title || paper.path }}
            </NuxtLink>
            <p class="mt-1 text-xs text-text/60">
              {{ paper.path }}
            </p>
          </div>

          <span
            v-if="paper.updated_at"
            class="shrink-0 text-xs text-text/60"
          >{{ paper.updated_at }}</span>
        </li>
      </ul>
    </AppCard>

    <UModal
      v-model:open="modalOpen"
      title="Analisar paper"
      @update:open="(v) => { if (!v && !uploading) closeModal(status?.done) }"
    >
      <template #body>
        <!-- sem status: dropzone -->
        <div v-if="!status">
          <div
            :class="[
              'border-2 border-dashed rounded-xl p-12 text-center cursor-pointer transition-colors',
              dragOver ? 'border-primary-500 bg-primary-500/10' : 'border-default hover:border-default/80',
            ]"
            @dragover.prevent="dragOver = true"
            @dragleave="dragOver = false"
            @drop.prevent="onDrop"
            @click="fileInput?.click()"
          >
            <UIcon name="i-heroicons-arrow-up-tray" class="w-10 h-10 text-text/40 mx-auto mb-3" />
            <p v-if="!file" class="text-text/60 text-sm">
              Arraste um PDF aqui ou clique para selecionar
            </p>
            <p v-else class="text-primary-400 font-medium text-sm">
              {{ file.name }}
            </p>
            <input
              ref="fileInput"
              type="file"
              accept=".pdf,application/pdf"
              class="hidden"
              @change="onFileChange"
            >
          </div>
          <div class="mt-4 flex justify-end gap-2">
            <UButton variant="ghost" @click="closeModal()">
              Cancelar
            </UButton>
            <UButton
              :disabled="!file || uploading"
              :loading="uploading"
              icon="i-heroicons-arrow-up-tray"
              @click="send"
            >
              Enviar
            </UButton>
          </div>
        </div>

        <!-- com status: progresso / sucesso / erro -->
        <div v-else class="py-4 space-y-4">
          <!-- erro -->
          <div v-if="status.error" class="text-center space-y-4">
            <UIcon name="i-heroicons-x-circle" class="w-12 h-12 text-red-500 mx-auto" />
            <p class="text-red-400 text-sm">
              {{ status.error }}
            </p>
            <UButton variant="outline" @click="reset(); file = null">
              Tentar novamente
            </UButton>
          </div>

          <!-- sucesso -->
          <div v-else-if="status.done" class="text-center space-y-4">
            <UIcon name="i-heroicons-check-circle" class="w-12 h-12 text-green-500 mx-auto" />
            <p class="text-green-400 font-medium">
              Paper anexado com sucesso!
            </p>
            <NuxtLink
              v-if="status.wiki_path"
              :to="`/wiki/${status.wiki_path.replace(/\.md$/, '')}`"
              class="text-primary-400 hover:underline text-sm"
            >
              Ver página na wiki →
            </NuxtLink>
            <div>
              <UButton @click="closeModal(true)">
                Fechar
              </UButton>
            </div>
          </div>

          <!-- em progresso -->
          <div v-else class="space-y-4">
            <p class="text-center text-sm text-text/60">
              Processando <span class="font-medium text-text">{{ file?.name }}</span>…
            </p>
            <div class="flex items-center gap-1">
              <div
                v-for="(step, i) in steps"
                :key="step"
                :class="[
                  'flex-1 h-2 rounded-full transition-colors',
                  i <= currentStepIndex() ? 'bg-primary-500' : 'bg-default',
                ]"
              />
            </div>
            <p class="text-center text-xs text-text/50">
              {{ stepLabels[status.step] ?? status.step }}…
            </p>
          </div>
        </div>
      </template>
    </UModal>
  </AppContainer>
</template>
