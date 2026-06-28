export default defineEventHandler((event) => {
  const jobId = event.context.params?.jobId ?? ''
  return proxyToBackend(event, `/api/ingest/status/${jobId}`)
})
