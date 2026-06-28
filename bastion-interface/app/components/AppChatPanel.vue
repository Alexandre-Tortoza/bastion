<script setup lang="ts">
import { marked } from 'marked'
import type { ChatMessage } from '~/types/bastion'

const props = defineProps<{
  messages: ChatMessage[]
  loading: boolean
}>()

const emit = defineEmits<{
  send: [query: string]
  'save-to-wiki': [message: ChatMessage]
  clear: []
}>()

const inputRef = ref<HTMLElement | null>(null)
const listRef = ref<HTMLElement | null>(null)
const draft = ref('')

function submit() {
  const q = draft.value.trim()
  if (!q) return
  emit('send', q)
  draft.value = ''
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    submit()
  }
}

watch(
  () => props.messages.map(msg => msg.content.length).join(':'),
  async () => {
    await nextTick()
    listRef.value?.lastElementChild?.scrollIntoView({ behavior: 'smooth' })
  }
)

function renderMd(text: string): string {
  return marked.parse(text, { breaks: true }) as string
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col overflow-hidden">
    <!-- message list -->
    <div ref="listRef" class="min-h-0 flex-1 space-y-4 overflow-y-auto p-4">
      <div v-if="messages.length === 0" class="text-center text-gray-500 mt-16">
        Faça uma pergunta sobre sua wiki acadêmica.
      </div>

      <div
        v-for="msg in messages"
        :key="msg.id"
        :class="['flex', msg.role === 'user' ? 'justify-end' : 'justify-start']"
      >
        <div
          :class="[
            'max-w-[80%] rounded-xl px-4 py-2 text-sm',
            msg.role === 'user' ? 'bg-primary-600 text-white' : 'bg-gray-800 text-gray-100',
          ]"
        >
          <!-- typing indicator -->
          <div v-if="msg.streaming && !msg.content" class="flex gap-1 py-1">
            <span
              class="w-2 h-2 rounded-full bg-gray-400 animate-bounce"
              style="animation-delay: 0ms"
            />
            <span
              class="w-2 h-2 rounded-full bg-gray-400 animate-bounce"
              style="animation-delay: 150ms"
            />
            <span
              class="w-2 h-2 rounded-full bg-gray-400 animate-bounce"
              style="animation-delay: 300ms"
            />
          </div>

          <!-- content -->
          <!-- eslint-disable-next-line vue/no-v-html -->
          <div
            v-else
            class="prose prose-invert prose-sm max-w-none"
            v-html="renderMd(msg.content)"
          />

          <!-- save button for assistant messages -->
          <div v-if="msg.role === 'assistant' && !msg.streaming" class="mt-2 flex justify-end">
            <UButton
              size="xs"
              variant="ghost"
              icon="i-heroicons-bookmark"
              @click="emit('save-to-wiki', msg)"
            >
              Salvar
            </UButton>
          </div>
        </div>
      </div>
    </div>

    <!-- input -->
    <div class="border-t border-gray-800 p-3 flex gap-2">
      <UInput
        ref="inputRef"
        v-model="draft"
        class="flex-1"
        placeholder="Pergunte sobre seus papers…"
        :disabled="loading"
        @keydown="onKeydown"
      />
      <UButton
        :disabled="loading || !draft.trim()"
        icon="i-heroicons-paper-airplane"
        @click="submit"
      />
      <UButton
        variant="ghost"
        icon="i-heroicons-trash"
        color="neutral"
        :disabled="messages.length === 0"
        @click="emit('clear')"
      />
    </div>
  </div>
</template>
