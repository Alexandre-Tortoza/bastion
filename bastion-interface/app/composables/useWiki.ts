import type { Decision, WikiPage } from '~/types/bastion'

export function useWikiPages(kind?: string) {
  return useAsyncData(
    `wiki-pages-${kind ?? 'all'}`,
    () => $fetch<{ pages: WikiPage[] }>('/api/wiki/pages', {
      query: kind ? { kind } : {},
    }),
  )
}

export function useWikiPage(path: Ref<string> | string) {
  const p = isRef(path) ? path : ref(path)
  return useAsyncData(
    () => `wiki-page-${p.value}`,
    () => $fetch<WikiPage>(`/api/wiki/pages/${p.value}`),
    { watch: [p] },
  )
}

export function useDecisions(status?: string) {
  return useAsyncData(
    `decisions-${status ?? 'all'}`,
    () => $fetch<{ decisions: Decision[] }>('/api/wiki/decisions', {
      query: status ? { status } : {},
    }),
  )
}
