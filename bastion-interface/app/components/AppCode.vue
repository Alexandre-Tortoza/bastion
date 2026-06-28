<script setup lang="ts">
import { EditorView, basicSetup } from 'codemirror'
import { EditorState } from '@codemirror/state'
import { oneDark } from '@codemirror/theme-one-dark'
import { latex } from 'codemirror-lang-latex'

const model = defineModel<string>({ default: '' })
const props = withDefaults(defineProps<{
  compiling?: boolean
  analyzing?: boolean
  saveLabel?: string
}>(), {
  compiling: false,
  analyzing: false,
  saveLabel: ''
})
const emit = defineEmits<{
  compile: [content: string]
  analyze: [content: string]
  blur: [content: string]
  change: [content: string]
}>()

const editorRef = ref<HTMLDivElement>()
const view = shallowRef<EditorView>()
let applyingExternalUpdate = false

const defaultDoc = `\\documentclass{article}
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

onMounted(() => {
  const startState = EditorState.create({
    doc: model.value || defaultDoc,
    extensions: [
      basicSetup,
      latex(),
      oneDark,
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          model.value = update.state.doc.toString()
          if (!applyingExternalUpdate) {
            emit('change', model.value)
          }
        }
      }),
      EditorView.domEventHandlers({
        blur() {
          emit('blur', model.value)
        }
      }),
      EditorView.lineWrapping
    ]
  })
  view.value = new EditorView({ state: startState, parent: editorRef.value! })
})

watch(model, (nextValue) => {
  const currentView = view.value
  if (!currentView || currentView.state.doc.toString() === nextValue) return

  applyingExternalUpdate = true
  currentView.dispatch({
    changes: {
      from: 0,
      to: currentView.state.doc.length,
      insert: nextValue
    }
  })
  applyingExternalUpdate = false
})

function compileNow() {
  emit('compile', model.value)
}

function analyzeNow() {
  emit('analyze', model.value)
}

onUnmounted(() => {
  view.value?.destroy()
})
</script>

<template>
  <div class="relative flex h-full min-h-0 flex-col bg-bg p-4 text-text">
    <div class="flex shrink-0 flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
      <div>
        <h2 class="text-lg font-semibold text-text">
          Editor LaTeX
        </h2>
        <span
          v-if="props.saveLabel"
          class="text-xs text-text/50"
        >
          {{ props.saveLabel }}
        </span>
      </div>

      <div class="flex gap-2">
        <AppButton
          icon="i-lucide-search"
          color="neutral"
          size="sm"
          :loading="props.analyzing"
          :disabled="props.analyzing"
          @click="analyzeNow"
        >
          Analisar
        </AppButton>
        <AppButton
          icon="i-lucide-play"
          color="primary"
          size="sm"
          :loading="props.compiling"
          :disabled="props.compiling"
          @click="compileNow"
        >
          Compilar
        </AppButton>
      </div>
    </div>
    <div
      ref="editorRef"
      class="mt-4 min-h-0 flex-1 overflow-hidden rounded-md border border-primary-800/80 shadow-[0_0_0_1px_rgba(3,115,140,0.15)]"
    />
  </div>
</template>

<style scoped>
:deep(.cm-editor) {
  height: 100%;
}

:deep(.cm-scroller) {
  height: 100%;
}
</style>
