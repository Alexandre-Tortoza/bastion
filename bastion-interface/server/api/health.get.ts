export default defineEventHandler(event =>
  proxyToBackend(event, '/api/health')
)
