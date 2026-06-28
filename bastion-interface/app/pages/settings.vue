<script setup lang="ts">
definePageMeta({ ssr: false })

const llmProvider = ref<'openai' | 'anthropic' | 'openrouter' | 'gemini' | ''>('')
const llmKey = ref('')
const llmModel = ref('')
const embedProvider = ref<'openai' | 'voyage' | 'gemini' | ''>('')
const embedKey = ref('')
const embedModel = ref('')

type LlmProvider = Exclude<typeof llmProvider.value, ''>
type EmbedProvider = Exclude<typeof embedProvider.value, ''>

const saved = ref(false)
const saving = ref(false)
const saveError = ref('')

const storageKey = 'bastion:settings'

const llmProviders: Array<{ label: string, value: LlmProvider, placeholder: string }> = [
  { label: 'Anthropic (Claude)', value: 'anthropic', placeholder: 'claude-sonnet-4-6' },
  { label: 'OpenAI (GPT)', value: 'openai', placeholder: 'gpt-4o' },
  { label: 'OpenRouter', value: 'openrouter', placeholder: 'openai/gpt-4o' },
  { label: 'Gemini', value: 'gemini', placeholder: 'gemini-2.0-flash' }
]

const embedProviders: Array<{ label: string, value: EmbedProvider, placeholder: string }> = [
  { label: 'Voyage AI', value: 'voyage', placeholder: 'voyage-3' },
  { label: 'OpenAI', value: 'openai', placeholder: 'text-embedding-3-small' },
  { label: 'Gemini', value: 'gemini', placeholder: 'text-embedding-004' }
]

const selectedLlmProvider = computed(() =>
  llmProviders.find(p => p.value === llmProvider.value)
)
const selectedEmbedProvider = computed(() =>
  embedProviders.find(p => p.value === embedProvider.value)
)

function loadFromStorage() {
  const stored = localStorage.getItem(storageKey)
  if (!stored) return null
  try {
    return JSON.parse(stored)
  } catch {
    localStorage.removeItem(storageKey)
    return null
  }
}

onMounted(async () => {
  const settings = loadFromStorage()
  if (!settings) return

  llmProvider.value = settings.llmProvider ?? ''
  llmKey.value = settings.llmKey ?? ''
  llmModel.value = settings.llmModel ?? ''
  embedProvider.value = settings.embedProvider ?? ''
  embedKey.value = settings.embedKey ?? ''
  embedModel.value = settings.embedModel ?? ''

  // Re-apply to backend in case it restarted
  if (llmProvider.value || embedProvider.value) {
    await applyToBackend().catch(() => null)
  }
})

async function applyToBackend() {
  return $fetch('/api/settings', {
    method: 'POST',
    body: {
      llmProvider: llmProvider.value || undefined,
      llmKey: llmKey.value || undefined,
      llmModel: llmModel.value || undefined,
      embedProvider: embedProvider.value || undefined,
      embedKey: embedKey.value || undefined,
      embedModel: embedModel.value || undefined
    }
  })
}

async function saveSettings() {
  saving.value = true
  saveError.value = ''

  localStorage.setItem(storageKey, JSON.stringify({
    llmProvider: llmProvider.value,
    llmKey: llmKey.value,
    llmModel: llmModel.value,
    embedProvider: embedProvider.value,
    embedKey: embedKey.value,
    embedModel: embedModel.value
  }))

  try {
    await applyToBackend()
    saved.value = true
    setTimeout(() => {
      saved.value = false
    }, 2500)
  } catch (e) {
    saveError.value = e instanceof Error ? e.message : 'Erro ao aplicar configurações'
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <UContainer class="h-full overflow-y-auto py-8 max-w-3xl">
    <div class="mb-8">
      <p class="text-sm font-medium text-primary mb-1">
        Sistema
      </p>
      <h1 class="text-2xl font-bold">
        Configurações
      </h1>
      <p class="mt-2 text-sm text-gray-400">
        Configure os providers de LLM e embeddings. As keys são salvas localmente e aplicadas ao backend automaticamente.
      </p>
    </div>

    <!-- LLM Card -->
    <UCard class="mb-6">
      <template #header>
        <h2 class="text-base font-semibold">
          LLM de consolidação
        </h2>
        <p class="text-sm text-gray-400 mt-1">
          Usada para ingestão de papers, chat e revisão LaTeX.
        </p>
      </template>

      <div class="space-y-4">
        <!-- Provider select -->
        <div>
          <label class="block text-sm font-medium text-gray-300 mb-1">Provider</label>
          <div class="flex flex-wrap gap-2">
            <UButton
              v-for="p in llmProviders"
              :key="p.value"
              :variant="llmProvider === p.value ? 'solid' : 'ghost'"
              size="sm"
              @click="llmProvider = p.value"
            >
              {{ p.label }}
            </UButton>
          </div>
        </div>

        <!-- API Key -->
        <div v-if="llmProvider">
          <label class="block text-sm font-medium text-gray-300 mb-1">API Key</label>
          <UInput
            v-model="llmKey"
            type="password"
            placeholder="Cole a API key"
            class="font-mono"
          />
        </div>

        <!-- Model -->
        <div v-if="llmProvider">
          <label class="block text-sm font-medium text-gray-300 mb-1">
            Modelo <span class="text-gray-500 font-normal">(opcional — default: {{ selectedLlmProvider?.placeholder }})</span>
          </label>
          <UInput
            v-model="llmModel"
            :placeholder="selectedLlmProvider?.placeholder ?? ''"
          />
        </div>
      </div>
    </UCard>

    <!-- Embeddings Card -->
    <UCard class="mb-6">
      <template #header>
        <h2 class="text-base font-semibold">
          Embeddings
        </h2>
        <p class="text-sm text-gray-400 mt-1">
          Habilita busca semântica híbrida. Sem embeddings, só busca por palavras-chave (FTS5).
        </p>
      </template>

      <div class="space-y-4">
        <div>
          <label class="block text-sm font-medium text-gray-300 mb-1">Provider</label>
          <div class="flex flex-wrap gap-2">
            <UButton
              v-for="p in embedProviders"
              :key="p.value"
              :variant="embedProvider === p.value ? 'solid' : 'ghost'"
              size="sm"
              @click="embedProvider = p.value"
            >
              {{ p.label }}
            </UButton>
          </div>
        </div>

        <div v-if="embedProvider">
          <label class="block text-sm font-medium text-gray-300 mb-1">API Key</label>
          <UInput
            v-model="embedKey"
            type="password"
            placeholder="Cole a API key"
            class="font-mono"
          />
        </div>

        <div v-if="embedProvider">
          <label class="block text-sm font-medium text-gray-300 mb-1">
            Modelo <span class="text-gray-500 font-normal">(opcional — default: {{ selectedEmbedProvider?.placeholder }})</span>
          </label>
          <UInput
            v-model="embedModel"
            :placeholder="selectedEmbedProvider?.placeholder ?? ''"
          />
        </div>
      </div>
    </UCard>

    <!-- Save -->
    <div class="flex items-center justify-end gap-3">
      <p
        v-if="saveError"
        class="text-sm text-red-400"
      >
        {{ saveError }}
      </p>
      <p
        v-if="saved"
        class="text-sm text-green-400"
      >
        Configurações salvas e aplicadas.
      </p>
      <UButton
        :loading="saving"
        icon="i-heroicons-check"
        @click="saveSettings"
      >
        Salvar configurações
      </UButton>
    </div>
  </UContainer>
</template>
