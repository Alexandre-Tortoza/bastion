<script setup lang="ts">
definePageMeta({ ssr: false })

const { upload, status, reset } = useIngest()

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

function retry() {
  reset()
  file.value = null
}
</script>

<template>
  <UContainer class="h-full overflow-y-auto py-12 max-w-lg">
    <h1 class="text-2xl font-bold mb-8 text-center">
      Ingerir Paper
    </h1>

    <!-- upload card -->
    <UCard v-if="!status">
      <!-- dropzone -->
      <div
        :class="[
          'border-2 border-dashed rounded-xl p-12 text-center cursor-pointer transition-colors',
          dragOver ? 'border-indigo-500 bg-indigo-500/10' : 'border-gray-700 hover:border-gray-500',
        ]"
        @dragover.prevent="dragOver = true"
        @dragleave="dragOver = false"
        @drop.prevent="onDrop"
        @click="fileInput?.click()"
      >
        <UIcon name="i-heroicons-arrow-up-tray" class="w-10 h-10 text-gray-500 mx-auto mb-3" />
        <p v-if="!file" class="text-gray-400 text-sm">
          Arraste um PDF aqui ou clique para selecionar
        </p>
        <p v-else class="text-indigo-400 font-medium text-sm">
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

      <div class="mt-4 flex justify-end">
        <UButton
          :disabled="!file || uploading"
          :loading="uploading"
          icon="i-heroicons-arrow-up-tray"
          @click="send"
        >
          Enviar
        </UButton>
      </div>
    </UCard>

    <!-- progress card -->
    <UCard v-else>
      <!-- error state -->
      <div v-if="status.error" class="text-center space-y-4 py-4">
        <UIcon name="i-heroicons-x-circle" class="w-12 h-12 text-red-500 mx-auto" />
        <p class="text-red-400 text-sm">
          {{ status.error }}
        </p>
        <UButton variant="outline" @click="retry">
          Tentar novamente
        </UButton>
      </div>

      <!-- success state -->
      <div v-else-if="status.done" class="text-center space-y-4 py-4">
        <UIcon name="i-heroicons-check-circle" class="w-12 h-12 text-green-500 mx-auto" />
        <p class="text-green-400 font-medium">
          Paper anexado com sucesso!
        </p>
        <NuxtLink
          v-if="status.wiki_path"
          :to="`/wiki/${status.wiki_path.replace(/\.md$/, '')}`"
          class="text-indigo-400 hover:underline text-sm"
        >
          Ver página na wiki →
        </NuxtLink>
        <div>
          <UButton variant="ghost" size="sm" @click="retry">
            Ingerir outro paper
          </UButton>
        </div>
      </div>

      <!-- in-progress state -->
      <div v-else class="py-4 space-y-4">
        <p class="text-center text-sm text-gray-400">
          Processando <span class="font-medium text-gray-200">{{ file?.name }}</span>…
        </p>

        <!-- step bar -->
        <div class="flex items-center gap-1">
          <template v-for="(step, i) in steps" :key="step">
            <div
              :class="[
                'flex-1 h-2 rounded-full transition-colors',
                i <= currentStepIndex() ? 'bg-indigo-500' : 'bg-gray-700',
              ]"
            />
          </template>
        </div>

        <p class="text-center text-xs text-gray-500">
          {{ stepLabels[status.step] ?? status.step }}…
        </p>
      </div>
    </UCard>
  </UContainer>
</template>
