import type { LatexDraft } from '../../utils/latexDraft'

export default defineEventHandler(async (event) => {
  const key = getLatexDraftKey()
  const draft = await useStorage<LatexDraft>('latex').getItem(key)

  return {
    draft: draft ?? null
  }
})
