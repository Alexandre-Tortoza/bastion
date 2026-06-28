<script setup lang="ts">
interface LatexDraft {
  source: string
  updatedAt: string
}

interface ReviewReference {
  path: string
  title: string
  kind?: string
  snippet: string
}

interface ReviewAnalyzeResponse {
  wiki_path: string
  suggestions_total: number
  suggestions: string
  references?: ReviewReference[]
}

type SaveStatus = 'idle' | 'dirty' | 'saving' | 'saved' | 'error'

const defaultSource = `\\documentclass{article}
\\usepackage[utf8]{inputenc}

\\title{Bastion Paper}
\\author{Author}
\\date{\\today}

\\begin{document}

\\maketitle

\\section{Introduction}

This is a sample LaTeX document.
Edit here to see changes in the preview.

\\end{document}`

const source = ref(defaultSource)
const pdfUrl = ref('')
const compileError = ref('')
const compiling = ref(false)
const pendingCompileSource = ref<string | null>(null)
const saveStatus = ref<SaveStatus>('idle')
const lastSavedAt = ref('')
const lastSavedSource = ref('')
let saveTimer: ReturnType<typeof setTimeout> | null = null

const analyzing = ref(false)
const analysisMarkdown = ref('')
const analysisReferences = ref<ReviewReference[]>([])
const showAnalysisModal = ref(false)

const analysisHtml = computed(() => markdownToHtml(analysisMarkdown.value))

const saveLabel = computed(() => {
  if (saveStatus.value === 'dirty') return 'Alterações pendentes'
  if (saveStatus.value === 'saving') return 'Salvando...'
  if (saveStatus.value === 'error') return 'Erro ao salvar'
  if (saveStatus.value === 'saved' && lastSavedAt.value) return `Salvo ${lastSavedAt.value}`
  return ''
})

function formatSavedAt(value: string) {
  return new Intl.DateTimeFormat('pt-BR', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  }).format(new Date(value))
}

function clearSaveTimer() {
  if (!saveTimer) return
  clearTimeout(saveTimer)
  saveTimer = null
}

function scheduleSave() {
  clearSaveTimer()
  saveTimer = setTimeout(() => {
    void saveDraft()
  }, 30_000)
}

async function readCompileError(response: Response) {
  const text = await response.text()

  try {
    const error = JSON.parse(text)
    return error.data?.message || error.statusMessage || text
  } catch {
    return text
  }
}

async function handleCompile(content: string) {
  if (compiling.value) {
    pendingCompileSource.value = content
    return true
  }

  compiling.value = true
  compileError.value = ''

  try {
    const response = await fetch('/api/latex/compile', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ source: content })
    })

    if (!response.ok) {
      throw new Error(await readCompileError(response))
    }

    const blob = await response.blob()
    const nextUrl = URL.createObjectURL(blob)

    if (pdfUrl.value) {
      URL.revokeObjectURL(pdfUrl.value)
    }

    pdfUrl.value = nextUrl
    return true
  } catch (error) {
    compileError.value = error instanceof Error ? error.message : 'Falha ao compilar o LaTeX.'
    return false
  } finally {
    compiling.value = false

    const nextSource = pendingCompileSource.value
    pendingCompileSource.value = null

    if (nextSource !== null && nextSource !== content) {
      void handleCompile(nextSource)
    }
  }
}

function analyzeFrontend(content: string): string {
  const lines = content.split('\n')
  const words = content.split(/\s+/).filter(Boolean).length
  const chars = content.length

  const sections = content.match(/\\(section|subsection|subsubsection)\{.*?\}/g)
  const equations = content.match(/\\begin\{equation\}/g)
  const figures = content.match(/\\includegraphics/g)
  const tables = content.match(/\\begin\{tabular\}/g)
  const citations = content.match(/\\cite\{.*?\}/g)


  let markdown = `### Análise do Documento\n\n`
  markdown += `- Palavras: **${words}**\n`
  markdown += `- Caracteres: **${chars}**\n`
  markdown += `- Linhas: **${lines.length}**\n`
  markdown += `- Seções: **${sections?.length ?? 0}**\n`
  markdown += `- Equações: **${equations?.length ?? 0}**\n`
  markdown += `- Figuras: **${figures?.length ?? 0}**\n`
  markdown += `- Tabelas: **${tables?.length ?? 0}**\n`
  markdown += `- Citações: **${citations?.length ?? 0}**\n`

  if (sections && sections.length > 0) {
    markdown += `\n#### Estrutura\n\n`
    for (const s of sections) {
      const label = s.replace(/\\(sub)?(sub)?section\{/, '').replace(/\}$/, '')
      markdown += `- ${label}\n`
    }
  }

  if (citations && citations.length > 0) {
    markdown += `\n#### Referências Citadas\n\n`
    const keys = [...new Set(citations.flatMap(c => c.replace(/\\cite\{/, '').replace(/\}$/, '').split(',').map(k => k.trim())))]
    markdown += keys.map(key => `- ${key}`).join('\n')
  }

  return markdown
}

function escapeHtml(value: string) {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;')
}

function inlineMarkdown(value: string) {
  return escapeHtml(value).replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
}

function markdownToHtml(markdown: string) {
  const lines = markdown.split('\n')
  let html = ''
  let listOpen = false

  function closeList() {
    if (!listOpen) return
    html += '</ul>'
    listOpen = false
  }

  for (const line of lines) {
    const trimmed = line.trim()

    if (!trimmed) {
      closeList()
      continue
    }

    if (trimmed.startsWith('### ')) {
      closeList()
      html += `<h3>${inlineMarkdown(trimmed.slice(4))}</h3>`
      continue
    }

    if (trimmed.startsWith('#### ')) {
      closeList()
      html += `<h4>${inlineMarkdown(trimmed.slice(5))}</h4>`
      continue
    }

    if (/^\d+\.\s+/.test(trimmed)) {
      closeList()
      html += `<p>${inlineMarkdown(trimmed)}</p>`
      continue
    }

    if (trimmed.startsWith('- ')) {
      if (!listOpen) {
        html += '<ul>'
        listOpen = true
      }
      html += `<li>${inlineMarkdown(trimmed.slice(2))}</li>`
      continue
    }

    closeList()
    html += `<p>${inlineMarkdown(trimmed)}</p>`
  }

  closeList()
  return html
}

async function syncToWiki(content: string) {
  try {
    await $fetch('/api/wiki/target', {
      method: 'PUT',
      body: { latex: content }
    })
  } catch {
    // silently ignore — wiki sync is best-effort
  }
}

async function handleAnalyze(content: string) {
  if (analyzing.value) return
  showAnalysisModal.value = true
  analyzing.value = true
  analysisMarkdown.value = ''
  analysisReferences.value = []

  await syncToWiki(content)

  try {
    const result = await $fetch<ReviewAnalyzeResponse>('/api/review/analyze', {
      method: 'POST',
      body: { latex: content }
    })
    analysisMarkdown.value = result.suggestions
    analysisReferences.value = result.references ?? []
  } catch {
    analysisMarkdown.value = analyzeFrontend(content)
  } finally {
    analyzing.value = false
  }
}

async function loadDraft() {
  try {
    const response = await $fetch<{ draft: LatexDraft | null }>('/api/latex/draft')

    if (response.draft) {
      source.value = response.draft.source
      lastSavedSource.value = response.draft.source
      lastSavedAt.value = formatSavedAt(response.draft.updatedAt)
      saveStatus.value = 'saved'
    }
    return true
  } catch {
    saveStatus.value = 'error'
    return true
  }
}

async function saveDraft(content = source.value) {
  clearSaveTimer()

  if (content === lastSavedSource.value) {
    return true
  }

  saveStatus.value = 'saving'

  try {
    const response = await $fetch<{ draft: LatexDraft }>('/api/latex/draft', {
      method: 'PUT',
      body: { source: content }
    })

    lastSavedSource.value = content
    lastSavedAt.value = formatSavedAt(response.draft.updatedAt)
    saveStatus.value = 'saved'
    void syncToWiki(content)
    return true
  } catch {
    saveStatus.value = 'error'
    return false
  }
}

function handleSourceChange(content: string) {
  if (content === lastSavedSource.value) return
  saveStatus.value = 'dirty'
  scheduleSave()
}

async function saveAndCompile(content: string) {
  const saved = await saveDraft(content)
  if (!saved) return
  await handleCompile(content)
}

onMounted(async () => {
  const loaded = await loadDraft()
  if (!loaded) return
  void handleCompile(source.value)
})

onUnmounted(() => {
  clearSaveTimer()
  if (pdfUrl.value) {
    URL.revokeObjectURL(pdfUrl.value)
  }
})
</script>

<template>
  <div class="h-full grid grid-cols-2 divide-x divide-default">
    <AppCode
      v-model="source"
      :compiling="compiling"
      :analyzing="analyzing"
      :save-label="saveLabel"
      @change="handleSourceChange"
      @blur="saveAndCompile"
      @compile="saveAndCompile"
      @analyze="handleAnalyze"
    />
    <AppPaperViewer
      :src="pdfUrl"
      :error="compileError"
      :loading="compiling"
    />

    <UModal v-model:open="showAnalysisModal" title="Análise do Paper">
      <template #body>
        <div class="max-h-[80vh] overflow-y-auto">
          <div
            v-if="analyzing"
            class="flex items-center justify-center py-12"
          >
            <UIcon name="i-heroicons-arrow-path" class="w-6 h-6 animate-spin text-gray-400" />
            <span class="ml-2 text-gray-400">Analisando...</span>
          </div>
          <div v-else class="space-y-5">
            <div
              class="analysis-markdown text-sm text-text/80"
              v-html="analysisHtml"
            />

            <div
              v-if="analysisReferences.length"
              class="rounded-lg border border-primary-800/70 bg-bg/60 p-4"
            >
              <h3 class="text-sm font-semibold text-text">
                Referências cruzadas na wiki
              </h3>
              <ul class="mt-3 space-y-3">
                <li
                  v-for="reference in analysisReferences"
                  :key="reference.path"
                  class="text-sm"
                >
                  <NuxtLink
                    :to="`/wiki/${reference.path.replace(/\.md$/, '')}`"
                    class="font-medium text-primary-300 hover:underline"
                  >
                    {{ reference.title || reference.path }}
                  </NuxtLink>
                  <p class="mt-1 text-xs text-text/60">
                    {{ reference.snippet }}
                  </p>
                </li>
              </ul>
            </div>
          </div>
        </div>
      </template>
    </UModal>
  </div>
</template>

<style scoped>
.analysis-markdown :deep(h3) {
  margin-bottom: 0.75rem;
  font-size: 1rem;
  font-weight: 700;
  color: var(--color-text);
}

.analysis-markdown :deep(h4) {
  margin-top: 1rem;
  margin-bottom: 0.5rem;
  font-size: 0.875rem;
  font-weight: 700;
  color: var(--color-primary-300);
}

.analysis-markdown :deep(p) {
  margin-bottom: 0.75rem;
  line-height: 1.7;
}

.analysis-markdown :deep(ul) {
  margin-bottom: 0.75rem;
  list-style: disc;
  padding-left: 1.25rem;
}

.analysis-markdown :deep(li) {
  margin-bottom: 0.35rem;
  line-height: 1.6;
}
</style>
