import type { ChatMessage } from '~/types/bastion'

const STORAGE_KEY = 'bastion:chat:history'
const MAX_MESSAGES = 100

function friendlyChatError(code?: string, message?: string, status?: number) {
  const text = (message ?? '').toLowerCase()

  if (code === 'BACKEND_UNAVAILABLE' || status === 503) {
    return 'Erro com o servidor: o backend Bastion não está respondendo. Verifique se ele está rodando.'
  }
  if (code === 'LLM_NOT_CONFIGURED') {
    return 'LLM não configurado. Abra Configurações, informe a API key e salve novamente.'
  }
  if (code === 'LLM_AUTH_ERROR' || status === 401 || status === 403) {
    return 'Erro na API key: a chave está inválida, expirada ou sem permissão para esse modelo.'
  }
  if (code === 'LLM_QUOTA_ERROR' || text.includes('quota') || text.includes('credit') || text.includes('billing')) {
    return 'Cota ou créditos esgotados no provider. Verifique billing, limites ou troque a chave/modelo.'
  }
  if (code === 'LLM_RATE_LIMITED' || status === 429) {
    return 'Rate limit atingido no provider. Aguarde alguns segundos e tente novamente.'
  }
  if (code === 'LLM_PROVIDER_UNAVAILABLE' || (status && status >= 500)) {
    return 'Erro temporário no provider de LLM. Tente novamente em instantes.'
  }
  if (message) {
    return `Erro no chat: ${message}`
  }
  return 'Erro inesperado no chat. Tente novamente e verifique os logs do servidor se persistir.'
}

async function readErrorResponse(response: Response) {
  try {
    const payload = await response.json()
    const data = payload.data ?? payload
    return friendlyChatError(
      data.code ?? payload.code,
      data.error ?? data.message ?? payload.message,
      response.status
    )
  } catch {
    return friendlyChatError(undefined, undefined, response.status)
  }
}

export function useChat() {
  const messages = ref<ChatMessage[]>([])
  const loading = ref(false)

  onMounted(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY)
      if (stored) messages.value = JSON.parse(stored)
    } catch {
      localStorage.removeItem(STORAGE_KEY)
    }
  })

  function saveToStorage() {
    try {
      const trimmed = messages.value.slice(-MAX_MESSAGES)
      localStorage.setItem(STORAGE_KEY, JSON.stringify(trimmed))
    } catch {
      return
    }
  }

  function clear() {
    messages.value = []
    try {
      localStorage.removeItem(STORAGE_KEY)
    } catch {
      return
    }
  }

  async function send(query: string) {
    if (loading.value || !query.trim()) return

    const userMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content: query.trim(),
      timestamp: new Date().toISOString()
    }
    messages.value.push(userMsg)

    const assistantMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'assistant',
      content: '',
      timestamp: new Date().toISOString(),
      streaming: true
    }
    messages.value.push(assistantMsg)
    const idx = messages.value.length - 1

    loading.value = true

    try {
      const response = await fetch('/api/chat/query', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ query })
      })

      if (!response.ok || !response.body) {
        messages.value[idx]!.content = await readErrorResponse(response)
        messages.value[idx]!.streaming = false
        return
      }

      const reader = response.body.getReader()
      const decoder = new TextDecoder()
      let buffer = ''
      let eventName = ''

      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        buffer += decoder.decode(value, { stream: true })

        const lines = buffer.split('\n')
        buffer = lines.pop() ?? ''

        for (const line of lines) {
          if (line.startsWith('event: ')) {
            eventName = line.slice(7).trim()
          } else if (line.startsWith('data: ')) {
            const raw = line.slice(6).trim()
            if (!raw || raw === '{}') continue
            try {
              const payload = JSON.parse(raw)
              if (eventName === 'token' && payload.text) {
                messages.value[idx]!.content += payload.text
              } else if (eventName === 'refs' && payload.refs) {
                messages.value[idx]!.refs = payload.refs
              } else if (eventName === 'error' && payload.error) {
                messages.value[idx]!.content = friendlyChatError(payload.code, payload.error)
              }
            } catch {
              continue
            }
          }
        }
      }
    } catch (e) {
      messages.value[idx]!.content = friendlyChatError(
        'BACKEND_UNAVAILABLE',
        e instanceof Error ? e.message : String(e)
      )
    } finally {
      messages.value[idx]!.streaming = false
      loading.value = false
      saveToStorage()
    }
  }

  return { messages, loading, send, clear }
}
