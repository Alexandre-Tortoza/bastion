<script setup lang="ts">
import type { ChatMessage, Reference } from '~/types/bastion'

definePageMeta({ ssr: false })

const { messages, loading, send, clear } = useChat()

const refs = computed<Reference[]>(() => {
  const last = [...messages.value].reverse().find(m => m.role === 'assistant' && m.refs?.length)
  return last?.refs ?? []
})

const saveModal = ref(false)
const messageToSave = ref<ChatMessage | null>(null)

function onSaveToWiki(msg: ChatMessage) {
  messageToSave.value = msg
  saveModal.value = true
}

const saving = ref(false)
async function confirmSave() {
  if (!messageToSave.value) return
  saving.value = true
  try {
    await $fetch('/api/wiki/pending', {
      method: 'POST',
      body: {
        title: 'Nota do Chat',
        pages_affected: [],
        justification: 'Salvo diretamente do chat',
        proposed_changes: messageToSave.value.content,
      },
    })
  } finally {
    saving.value = false
    saveModal.value = false
    messageToSave.value = null
  }
}
</script>

<template>
  <div class="grid h-[calc(100vh-4rem)] min-h-0 grid-cols-12 gap-0">
    <!-- chat panel -->
    <div class="col-span-12 h-full min-h-0 border-r border-gray-800 lg:col-span-8">
      <AppChatPanel
        :messages="messages"
        :loading="loading"
        @send="send"
        @save-to-wiki="onSaveToWiki"
        @clear="clear"
      />
    </div>

    <!-- references -->
    <div class="col-span-12 h-full min-h-0 lg:col-span-4">
      <AppReferencePanel
        :references="refs"
      />
    </div>
  </div>

  <!-- save modal -->
  <UModal v-model:open="saveModal" title="Salvar na Wiki">
    <template #body>
      <p class="text-sm text-gray-400">
        A mensagem será salva como proposta de consolidação em <code>_pending/</code>.
      </p>
    </template>
    <template #footer>
      <div class="flex gap-2 justify-end">
        <UButton variant="ghost" @click="saveModal = false">
          Cancelar
        </UButton>
        <UButton :loading="saving" @click="confirmSave">
          Salvar
        </UButton>
      </div>
    </template>
  </UModal>
</template>
