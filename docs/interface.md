# Bastion — Interface Web

> Spec implementável do frontend Nuxt 3: todas as rotas, tipos TypeScript,
> props/emits dos componentes, protocolo SSE do chat, rotas Nitro, composables
> e convenções visuais.
>
> A lógica de busca e retrieval está em `knowledge-layer.md`. O contrato das
> rotas do backend Rust está em `general.md §10`. O formato das páginas wiki
> está em `wiki.md`.

---

## 1. `nuxt.config.ts`

Partindo do skeleton existente em `bastion-front/nuxt.config.ts`:

```typescript
export default defineNuxtConfig({
  compatibilityDate: '2025-07-15',
  devtools: { enabled: true },

  modules: ['@nuxt/ui'],

  runtimeConfig: {
    bastionBackendUrl: process.env.BASTION_BACKEND_URL ?? 'http://localhost:8080',
    bastionApiToken: process.env.BASTION_API_TOKEN ?? '',
    public: {},
  },
})
```

`@nuxt/ui` inclui Tailwind CSS, Tailwind Typography e os componentes `U*`.
`runtimeConfig` (não `public`) — os valores são usados apenas server-side no
Nitro; o browser nunca vê o token.

**`app/app.vue`** deve ser trocado para `<NuxtPage />`:
```vue
<template>
  <NuxtLayout>
    <NuxtPage />
  </NuxtLayout>
</template>
```

---

## 2. `server/utils/backend.ts`

Utilitário único para todas as rotas Nitro. Lê as configurações de runtime e
injeta o token em todo `fetch()` para o Rust.

```typescript
import { H3Event, sendStream } from 'h3'

interface BackendOptions {
  method?: 'GET' | 'POST' | 'PUT' | 'DELETE'
  body?: unknown
  query?: Record<string, string | number | boolean>
  stream?: boolean
}

export async function proxyToBackend(
  event: H3Event,
  path: string,
  opts: BackendOptions = {}
) {
  const config = useRuntimeConfig()
  const url = `${config.bastionBackendUrl}${path}`
  const headers: Record<string, string> = {
    Authorization: `Bearer ${config.bastionApiToken}`,
    'Content-Type': 'application/json',
  }

  if (opts.stream) {
    const response = await fetch(url, {
      method: opts.method ?? 'GET',
      headers,
      body: opts.body ? JSON.stringify(opts.body) : undefined,
    })
    setHeader(event, 'Content-Type', 'text/event-stream')
    setHeader(event, 'Cache-Control', 'no-cache')
    return sendStream(event, response.body!)
  }

  return $fetch(url, {
    method: opts.method ?? 'GET',
    headers,
    body: opts.body,
    query: opts.query,
  })
}
```

---

## 3. Tipos TypeScript Compartilhados

`types/bastion.d.ts` — importado automaticamente pelo Nuxt:

```typescript
export interface WikiPage {
  path: string             // relativo a wiki/, sem .md  ex: "papers/vaswani-2017-attention"
  title: string
  kind: 'paper' | 'concept' | 'method' | 'decision' | 'comparison' | 'synthesis' | 'review'
  tier: 'semantic' | 'episodic' | 'working'
  created_at: string       // ISO date YYYY-MM-DD
  updated_at: string
  pinned: boolean
  tags: string[]
  status?: string          // para papers e decisions
  // body e links apenas no endpoint de página individual
  body?: string            // HTML renderizado do markdown
  wikilinks?: WikiLink[]
  backlinks?: WikiLink[]
}

export interface WikiLink {
  path: string
  title: string
  label?: string
}

export interface Decision extends WikiPage {
  kind: 'decision'
  decision_status: 'proposed' | 'accepted' | 'superseded' | 'rejected'
  date: string
  context_excerpt: string  // primeiros ~150 chars da seção Context
  related_papers: string[] // caminhos wiki
}

export interface Reference {
  kind: 'paper' | 'wiki'
  title: string
  excerpt: string          // HTML com <mark> nos termos buscados
  page?: number            // número de página no PDF (apenas papers)
  pdf_url?: string         // /raw/papers/<name>/original.pdf#page=N
  wiki_path?: string       // caminho wiki para o kind='wiki'
}

export interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string          // texto completo; cresce token a token durante streaming
  refs?: Reference[]       // preenchido após o evento 'refs' do SSE
  timestamp: string        // ISO datetime
  streaming?: boolean      // true enquanto tokens estão chegando
}

export interface Suggestion {
  id: string
  location: string         // ex: "Seção 3.2, parágrafo 1"
  found_in_wiki: string    // o que a wiki diz sobre o tópico
  suggested_change: string // o que adicionar, modificar ou citar
  wiki_ref: string         // wikilink para a página fonte
  severity: 'info' | 'warning' | 'error'
}

export interface IngestStatus {
  job_id: string
  step: 'received' | 'converting' | 'extracting' | 'integrating' | 'indexed'
  done: boolean
  error?: string
  wiki_path?: string       // preenchido quando done=true e sucesso
}
```

---

## 4. Páginas

| Rota | Arquivo | SSR | Layout | Componentes principais |
|---|---|---|---|---|
| `/` | `pages/index.vue` | sim | `default` | cards de papers recentes, decisões abertas, log |
| `/editor` | `pages/editor.vue` | não | `editor` | LaTeXEditor, ReferencePanel, SuggestionList |
| `/chat` | `pages/chat.vue` | não | `default` | ChatPanel, ReferencePanel |
| `/wiki` | `pages/wiki/index.vue` | sim | `default` | WikiTree + renderer markdown |
| `/wiki/papers` | `pages/wiki/papers.vue` | sim | `default` | tabela + tabs por status |
| `/wiki/decisions` | `pages/wiki/decisions.vue` | sim | `default` | DecisionCard list + tabs |
| `/ingest` | `pages/ingest.vue` | não | `default` | dropzone + barra de progresso |
| `/login` | `pages/login.vue` | não | `blank` | form de token |

### `/` — Dashboard

Grid 12 colunas. Coluna esquerda (8): "Papers Recentes" (últimos 5, card list)
e alertas de lint se existirem. Coluna direita (4): contagem de decisões
`proposed` com lista e "Última Atividade" (últimas 5 entradas do log).

```typescript
// useAsyncData em setup
const { data: recentPapers } = await useAsyncData('recent-papers', () =>
  $fetch('/api/wiki/pages', { query: { kind: 'paper', sort: 'updated_at', limit: 5 } })
)
const { data: openDecisions } = await useAsyncData('open-decisions', () =>
  $fetch('/api/wiki/decisions', { query: { status: 'proposed' } })
)
const { data: logEntries } = await useAsyncData('log', () =>
  $fetch('/api/wiki/log', { query: { limit: 5 } })
)
```

Empty state (primeiro acesso, nenhum paper): card prominente "Envie seu primeiro
artigo" com link para `/ingest`.

### `/editor` — Editor LaTeX

`definePageMeta({ ssr: false })`. Três painéis: esquerda (6 cols) `LaTeXEditor`,
centro (3 cols) preview markdown, direita (3 cols) `ReferencePanel`. Botão
"Revisar" no header: envia conteúdo para `POST /api/review/analyze` e abre
slide-over com `SuggestionList`.

Estado: conteúdo LaTeX em `ref<string>` (não persistido entre sessões em v1).
Sugestões em `ref<Suggestion[]>` resetadas a cada nova revisão.

### `/chat` — Chat

`definePageMeta({ ssr: false })`. Dois painéis: esquerda (8 cols) `ChatPanel`,
direita (4 cols) `ReferencePanel` (vazio até uma resposta com `refs` chegar).
Botão "Salvar na wiki" aparece em mensagens do assistant; abre modal confirmando
antes de chamar `POST /api/wiki/pending`.

Histórico: salvo/carregado de `localStorage` com chave `bastion:chat:history`.
Array de `ChatMessage[]`, máximo 100 mensagens (mais antigas são removidas do
início).

### `/wiki` — Wiki Browser

SSR habilitado. Dois painéis: esquerda (3 cols) `WikiTree`, direita (9 cols)
conteúdo da página wiki renderizado como markdown. Página inicial: conteúdo de
`wiki/index.md`.

Clicar em um nó do `WikiTree` chama `GET /api/wiki/pages/<path>` e renderiza
no painel direito — sem navegação de página completa. Wikilinks no markdown
renderizado emitem evento para trocar a página no painel direito.

### `/wiki/papers` — Lista de Papers

Tabela com colunas: título (link para abrir wiki page em slide-over), autores,
ano, venue, status badge. Filtros por status via tabs (Todos / Ingerido /
Revisado / Supersedido).

### `/wiki/decisions` — Lista de Decisões

Tabs por status: Propostas / Aceitas / Supersedidas / Rejeitadas. Cada item é
um `DecisionCard`.

### `/ingest` — Upload de PDF

`definePageMeta({ ssr: false })`. Card centralizado com dropzone (drag-and-drop
+ file picker). Após upload: barra de progresso com 5 etapas. Polling de status
a cada 2 segundos via `useIngest`. Ao concluir: link "Ver página na wiki".

### `/login` — Login

`definePageMeta({ layout: false })`. Card centralizado sem layout de app. Campo
de texto para token + botão "Conectar". Chama `POST /api/auth/login`. Sucesso →
redireciona para `/`. Falha → mensagem de erro inline.

---

## 5. Componentes

### `LaTeXEditor.vue`

```typescript
// Props
const props = defineProps<{
  modelValue: string
  readOnly?: boolean         // default: false
  placeholder?: string
}>()
// Emits
const emit = defineEmits<{
  'update:modelValue': [value: string]
  'review': []               // acionado por Ctrl+Enter
}>()
```

CodeMirror 6 via `vue-codemirror`. Extensão de linguagem LaTeX aplicada. Emit
de `update:modelValue` com debounce de 300ms. Ctrl+Enter emite `review`. Sem
preview embutido — o pai renderiza o preview.

### `ChatPanel.vue`

```typescript
defineProps<{
  messages: ChatMessage[]
  loading: boolean
}>()
defineEmits<{
  'send': [query: string]
  'save-to-wiki': [message: ChatMessage]
  'clear': []
}>()
```

Lista de mensagens em container com `overflow-y-auto`. Auto-scroll para o final
em nova mensagem. Mensagens do assistant renderizam markdown (via
`@nuxtjs/mdc` ou `marked`) com suporte a wikilinks: `[[path|label]]` vira um
link que emite evento para abrir a página wiki em modal. Ícone "Salvar" em
mensagens do assistant emite `save-to-wiki`. Spinner de "digitando" enquanto
`streaming: true`.

### `ReferencePanel.vue`

```typescript
defineProps<{
  references: Reference[]
}>()
defineEmits<{
  'open-pdf': [url: string, page: number]
  'open-wiki': [path: string]
}>()
```

Empty state: "Nenhuma referência ainda — faça uma pergunta no chat." Cada card:
título, excerpt (HTML com `<mark>` styled como highlight), e para papers o link
"Ver p. N" que emite `open-pdf`.

### `WikiTree.vue`

```typescript
defineProps<{
  pages: WikiPage[]
  selectedPath?: string
}>()
defineEmits<{
  'select': [page: WikiPage]
}>()
```

Agrupa páginas por diretório. Cada grupo é uma seção colapsável. Cada item
mostra:
- Dot colorido por tier: semantic=`bg-green-500`, episodic=`bg-yellow-500`
- Ícone por kind (usar Heroicons)
- Badge de status para decisions

### `DecisionCard.vue`

```typescript
defineProps<{
  decision: Decision
  compact?: boolean          // default: false; omite context_excerpt quando true
}>()
defineEmits<{
  'open': [decision: Decision]
}>()
```

Cor do badge por `decision_status`: proposed=`blue`, accepted=`green`,
superseded=`gray`, rejected=`red` (usar `UBadge` do Nuxt UI).

### `PdfViewer.vue`

```typescript
defineProps<{
  url: string                // ex: /raw/papers/vaswani-2017-attention/original.pdf#page=7
  page?: number              // default: 1 (usado se url não tem #page=N)
}>()
```

`<iframe :src="url" width="100%" height="100%">`. Link de fallback "Abrir PDF
em nova aba" abaixo do iframe para ambientes onde iframe PDF falha. Sem
dependência de PDF.js em v1.

### `SuggestionList.vue`

```typescript
defineProps<{
  suggestions: Suggestion[]
  loading: boolean
}>()
defineEmits<{
  'accept': [suggestion: Suggestion]
  'discard': [suggestion: Suggestion]
  'accept-all': []
}>()
```

Cada item: card com borda colorida por severity (info=blue, warning=yellow,
error=red). Mostra `location`, `found_in_wiki` e `suggested_change`. Botões
"Aceitar" e "Descartar". Botão "Aceitar Todos" no header. Empty state: "Nenhuma
sugestão. Clique em Revisar para checar seu LaTeX contra a wiki."

---

## 6. Rotas Nitro (Proxies)

Arquivos em `server/api/`. Cada arquivo é um proxy fino — validação de input
quando necessário, `proxyToBackend(event, ...)`, retornar resultado.

| Arquivo Nitro | Rota Rust |
|---|---|
| `server/api/wiki/pages.get.ts` | `GET /api/wiki/pages` (repassa query params) |
| `server/api/wiki/pages/[...path].get.ts` | `GET /api/wiki/pages/*path` |
| `server/api/wiki/decisions.get.ts` | `GET /api/wiki/decisions` |
| `server/api/wiki/log.get.ts` | `GET /api/wiki/log` |
| `server/api/wiki/pending.post.ts` | `POST /api/wiki/pending` |
| `server/api/chat/query.post.ts` | `POST /api/chat/query` — SSE stream |
| `server/api/ingest/upload.post.ts` | `POST /api/ingest/upload` — multipart |
| `server/api/ingest/status/[jobId].get.ts` | `GET /api/ingest/status/:jobId` |
| `server/api/review/analyze.post.ts` | `POST /api/review/analyze` |
| `server/api/auth/login.post.ts` | **Não proxied** — validado no Nitro, seta cookie |
| `server/middleware/auth.ts` | Middleware: valida cookie, protege todas as rotas `/api/**` exceto `/api/auth/login` |

**Exemplo de rota SSE (`server/api/chat/query.post.ts`):**
```typescript
export default defineEventHandler(async (event) => {
  const body = await readBody(event)
  return proxyToBackend(event, '/api/chat/query', {
    method: 'POST',
    body,
    stream: true,
  })
})
```

---

## 7. Protocolo SSE do Chat

Chat usa Server-Sent Events (não WebSocket). SSE simplifica o proxy no Nitro e
permite `fetch()` + `ReadableStream` no cliente.

**Request:**
```
POST /api/chat/query
Content-Type: application/json

{
  "query": "O que sabemos sobre regularização?",
  "history": [/* últimos N ChatMessage com role e content */]
}
```

**Response:** `Content-Type: text/event-stream`

```
event: token
data: {"token": "A "}

event: token
data: {"token": "principal "}

event: refs
data: {"refs": [{"kind": "paper", "title": "...", "excerpt": "...", "page": 7, "pdf_url": "/raw/...#page=7"}]}

event: done
data: {}

event: error
data: {"error": "LLM provider error: rate limit exceeded"}
```

O evento `refs` é enviado uma vez, após o último `token`. O evento `error` é
enviado em vez de `done` quando algo falha — o cliente deve sempre registrar
listener para `error` antes de abrir o stream.

**Consumo client-side (em `useChat.ts`):**

Usar `fetch()` com `response.body.getReader()` e `TextDecoder`. **Não usar
`EventSource`** — não suporta método POST.

```typescript
const reader = response.body!.getReader()
const decoder = new TextDecoder()
let buffer = ''

while (true) {
  const { done, value } = await reader.read()
  if (done) break
  buffer += decoder.decode(value, { stream: true })
  const lines = buffer.split('\n')
  buffer = lines.pop() ?? ''
  for (const line of lines) {
    if (line.startsWith('data: ')) {
      const payload = JSON.parse(line.slice(6))
      // processar por event type
    }
  }
}
```

---

## 8. Composables

### `composables/useWiki.ts`

```typescript
// Lista de todas as páginas (cacheada)
export function useWikiPages(kind?: string) {
  return useAsyncData(
    `wiki-pages-${kind ?? 'all'}`,
    () => $fetch<WikiPage[]>('/api/wiki/pages', { query: kind ? { kind } : {} })
  )
}

// Conteúdo de uma página específica
export function useWikiPage(path: Ref<string> | string) {
  const p = isRef(path) ? path : ref(path)
  return useAsyncData(
    () => `wiki-page-${p.value}`,
    () => $fetch<WikiPage>(`/api/wiki/pages/${p.value}`),
    { watch: [p] }
  )
}

// Lista de decisões
export function useDecisions(status?: string) {
  return useAsyncData(
    `decisions-${status ?? 'all'}`,
    () => $fetch<Decision[]>('/api/wiki/decisions', { query: status ? { status } : {} })
  )
}
```

### `composables/useChat.ts`

```typescript
export function useChat() {
  const STORAGE_KEY = 'bastion:chat:history'
  const MAX_MESSAGES = 100

  const messages = ref<ChatMessage[]>([])
  const loading = ref(false)

  // Carregar histórico ao montar
  onMounted(() => {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (stored) messages.value = JSON.parse(stored)
  })

  async function send(query: string) {
    // 1. Append user message
    // 2. Criar assistant message com streaming: true
    // 3. Abrir SSE stream
    // 4. Montar tokens em message.content
    // 5. Ao 'refs': message.refs = payload.refs
    // 6. Ao 'done': message.streaming = false; salvar localStorage
    // 7. Ao 'error': message.content = `Erro: ${payload.error}`; message.streaming = false
  }

  function saveToStorage() {
    const trimmed = messages.value.slice(-MAX_MESSAGES)
    localStorage.setItem(STORAGE_KEY, JSON.stringify(trimmed))
  }

  return { messages, loading, send }
}
```

### `composables/useIngest.ts`

```typescript
interface UseIngestReturn {
  upload: (file: File) => Promise<{ jobId: string }>
  status: Ref<IngestStatus | null>
  reset: () => void
}

export function useIngest(): UseIngestReturn {
  const status = ref<IngestStatus | null>(null)
  let pollInterval: ReturnType<typeof setInterval> | null = null

  async function upload(file: File) {
    const form = new FormData()
    form.append('file', file)
    const { job_id } = await $fetch<{ job_id: string }>('/api/ingest/upload', {
      method: 'POST',
      body: form,
    })
    startPolling(job_id)
    return { jobId: job_id }
  }

  function startPolling(jobId: string) {
    pollInterval = setInterval(async () => {
      const s = await $fetch<IngestStatus>(`/api/ingest/status/${jobId}`)
      status.value = s
      if (s.done || s.error) {
        clearInterval(pollInterval!)
        pollInterval = null
      }
    }, 2000)
  }

  function reset() {
    if (pollInterval) clearInterval(pollInterval)
    status.value = null
  }

  return { upload, status, reset }
}
```

---

## 9. Convenções Visuais

### Cores e superfícies

```
bg-gray-950 text-gray-100       ← fundo da aplicação
rounded-xl border border-gray-800 bg-gray-900  ← painéis e cards
indigo-500 / hover:indigo-600   ← elementos interativos (botões, links)
```

### Conteúdo wiki (markdown)

```html
<div class="prose prose-invert prose-sm max-w-none">
  <!-- HTML renderizado do markdown -->
</div>
```

### Dots de tier

```html
<span class="inline-block w-2 h-2 rounded-full bg-green-500" />  <!-- semantic -->
<span class="inline-block w-2 h-2 rounded-full bg-yellow-500" /> <!-- episodic -->
<span class="inline-block w-2 h-2 rounded-full bg-gray-500" />   <!-- working -->
```

### Badges de status (Nuxt UI `UBadge`)

| Status | `color` prop |
|---|---|
| `proposed` | `blue` |
| `accepted` | `green` |
| `superseded` | `gray` |
| `rejected` | `red` |
| `ingested` | `indigo` |
| `reviewed` | `green` |

### Layouts multi-pane

```html
<div class="grid grid-cols-12 gap-4 h-full">
  <div class="col-span-3 overflow-y-auto h-full"><!-- painel esquerdo --></div>
  <div class="col-span-9 overflow-y-auto h-full"><!-- painel direito --></div>
</div>
```

Cada painel define seu próprio `overflow-y-auto h-full` para scroll independente.

### Ícones

Usar `@nuxt/icon` (incluído com `@nuxt/ui`) com o set Heroicons:
```html
<UIcon name="i-heroicons-document-text" />
<UIcon name="i-heroicons-chat-bubble-left-right" />
<UIcon name="i-heroicons-arrow-up-tray" />
```

---

## 10. Middleware de Auth (Nitro)

`server/middleware/auth.ts` — protege todas as rotas Nitro exceto login:

```typescript
export default defineEventHandler((event) => {
  const path = getRequestURL(event).pathname
  if (path === '/api/auth/login' || path === '/api/health') return
  if (!path.startsWith('/api/')) return  // páginas Vue não precisam de auth

  const token = getCookie(event, 'bastion_token')
  if (!token) {
    return sendRedirect(event, '/login', 302)
  }
  // O token é injetado no header pelo proxyToBackend; o Rust valida lá
})
```

`server/api/auth/login.post.ts` — validado no Nitro, sem proxy para o Rust:

```typescript
export default defineEventHandler(async (event) => {
  const body = await readBody<{ token: string }>(event)
  const config = useRuntimeConfig()
  
  if (body.token !== config.bastionApiToken) {
    throw createError({ statusCode: 401, message: 'Token inválido' })
  }
  
  setCookie(event, 'bastion_token', body.token, {
    httpOnly: true,
    sameSite: 'strict',
    maxAge: 60 * 60 * 24 * 30, // 30 dias
  })
  
  return { ok: true }
})
```
