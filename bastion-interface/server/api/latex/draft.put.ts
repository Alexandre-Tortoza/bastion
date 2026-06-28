import type { LatexDraft } from '../../utils/latexDraft'

export default defineEventHandler(async (event) => {
  const body = await readBody<{ source?: unknown }>(event)
  const source = validateLatexDraftSource(body?.source)
  const draft: LatexDraft = {
    source,
    updatedAt: new Date().toISOString()
  }

  await useStorage<LatexDraft>('latex').setItem(getLatexDraftKey(), draft)

  return { draft }
})
