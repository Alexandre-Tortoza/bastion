export default defineEventHandler(async (event) => {
  return proxyToBackend(event, '/api/lint/run', { method: 'POST' })
})
