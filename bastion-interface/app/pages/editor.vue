<script setup lang="ts">
definePageMeta({ ssr: false })

const source = ref(`\\documentclass{article}
\\usepackage[utf8]{inputenc}

\\title{Meu Paper}
\\author{Autor}
\\date{\\today}

\\begin{document}
\\maketitle

\\section{Introdução}

Escreva seu texto LaTeX aqui.

\\end{document}`)

const pdfUrl = ref('')
const compileError = ref('')
const compiling = ref(false)

const suggestionsRaw = ref('')
const reviewing = ref(false)
const reviewWikiPath = ref('')

async function handleCompile(content: string) {
  compiling.value = true
  compileError.value = ''
  try {
    const response = await fetch('/api/latex/compile', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ source: content }),
    })
    if (!response.ok) {
      const text = await response.text()
      try { compileError.value = JSON.parse(text)?.data?.message ?? text }
      catch { compileError.value = text }
      return
    }
    const blob = await response.blob()
    if (pdfUrl.value) URL.revokeObjectURL(pdfUrl.value)
    pdfUrl.value = URL.createObjectURL(blob)
  } catch (e) {
    compileError.value = e instanceof Error ? e.message : 'Erro ao compilar'
  } finally {
    compiling.value = false
  }
}

async function handleReview() {
  reviewing.value = true
  suggestionsRaw.value = ''
  reviewWikiPath.value = ''
  try {
    const result = await $fetch<{ suggestions: string; wiki_path: string }>('/api/review/analyze', {
      method: 'POST',
      body: { latex: source.value },
    })
    suggestionsRaw.value = result.suggestions
    reviewWikiPath.value = result.wiki_path
  } catch (e) {
    suggestionsRaw.value = `Erro: ${e instanceof Error ? e.message : e}`
  } finally {
    reviewing.value = false
  }
}

onUnmounted(() => { if (pdfUrl.value) URL.revokeObjectURL(pdfUrl.value) })
</script>

<template>
  <div class="h-[calc(100vh-4rem)] flex flex-col">
    <!-- toolbar -->
    <div class="flex items-center gap-2 px-4 py-2 border-b border-gray-800">
      <UButton
        :loading="compiling"
        size="sm"
        variant="outline"
        icon="i-heroicons-play"
        @click="handleCompile(source)"
      >
        Compilar
      </UButton>
      <UButton
        :loading="reviewing"
        size="sm"
        icon="i-heroicons-magnifying-glass"
        @click="handleReview"
      >
        Revisar
      </UButton>
      <NuxtLink
        v-if="reviewWikiPath"
        :to="`/wiki/${reviewWikiPath.replace(/\.md$/, '')}`"
        class="text-xs text-indigo-400 hover:underline ml-2"
      >
        Ver revisão na wiki →
      </NuxtLink>
    </div>

    <!-- three-panel layout -->
    <div class="flex-1 grid grid-cols-12 min-h-0">
      <!-- editor -->
      <div class="col-span-6 border-r border-gray-800 h-full overflow-hidden">
        <AppCode
          v-model="source"
          :compiling="compiling"
          @compile="handleCompile"
        />
      </div>

      <!-- pdf preview -->
      <div class="col-span-3 border-r border-gray-800 h-full">
        <AppPaperViewer
          :src="pdfUrl"
          :error="compileError"
          :loading="compiling"
        />
      </div>

      <!-- suggestions -->
      <div class="col-span-3 h-full">
        <AppSuggestionList
          :suggestions-raw="suggestionsRaw"
          :loading="reviewing"
        />
      </div>
    </div>
  </div>
</template>
