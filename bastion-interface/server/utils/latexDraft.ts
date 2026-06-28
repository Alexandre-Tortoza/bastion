import { createError } from 'h3'

export const maxLatexDraftBytes = 2 * 1024 * 1024

export interface LatexDraft {
  source: string
  updatedAt: string
}

export function getLatexDraftKey() {
  return 'drafts:default'
}

export function validateLatexDraftSource(source: unknown) {
  if (typeof source !== 'string') {
    throw createError({ statusCode: 400, statusMessage: 'LaTeX source is required' })
  }

  if (Buffer.byteLength(source, 'utf8') > maxLatexDraftBytes) {
    throw createError({ statusCode: 413, statusMessage: 'LaTeX source is too large' })
  }

  return source
}
