export default defineEventHandler(async (event) => {
  return proxyToBackend(event, '/api/embeddings/backfill', { method: 'POST' })
})
