import type { IngestStatus } from '~/types/bastion'

function friendlyUploadError(err: unknown): string {
  const e = err as Record<string, unknown>
  const msg = (e?.message as string | undefined) ?? ''
  const code = e?.statusCode as number | undefined

  if (msg.includes('fetch failed') || msg.includes('no response') || msg.includes('ECONNREFUSED'))
    return 'O servidor não está disponível. Verifique se o bastion-web está rodando.'
  if (code === 413)
    return 'Arquivo muito grande.'
  if (code === 415 || msg.includes('unsupported'))
    return 'Formato não suportado. Envie apenas arquivos PDF.'
  if (code === 500)
    return 'Erro interno do servidor. Tente novamente.'

  const readable = (e?.data as Record<string, unknown>)?.message as string | undefined
  return readable ?? (e?.statusMessage as string | undefined) ?? 'Falha ao enviar o arquivo. Tente novamente.'
}

export function useIngest() {
  const status = ref<IngestStatus | null>(null)
  let pollInterval: ReturnType<typeof setInterval> | null = null

  async function upload(file: File): Promise<{ jobId: string }> {
    status.value = null
    const form = new FormData()
    form.append('file', file)

    try {
      const result = await $fetch<{ job_id: string }>('/api/ingest/upload', {
        method: 'POST',
        body: form,
      })
      startPolling(result.job_id)
      return { jobId: result.job_id }
    } catch (err) {
      status.value = { job_id: '', step: 'received', done: false, error: friendlyUploadError(err) }
      throw err
    }
  }

  function startPolling(jobId: string) {
    if (pollInterval) clearInterval(pollInterval)
    pollInterval = setInterval(async () => {
      try {
        const s = await $fetch<IngestStatus>(`/api/ingest/status/${jobId}`)
        status.value = s
        if (s.done || s.error) {
          clearInterval(pollInterval!)
          pollInterval = null
        }
      } catch {}
    }, 2000)
  }

  function reset() {
    if (pollInterval) { clearInterval(pollInterval); pollInterval = null }
    status.value = null
  }

  onUnmounted(reset)

  return { upload, status, reset }
}
