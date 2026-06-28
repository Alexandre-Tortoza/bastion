export default defineEventHandler(async (event) => {
  const config = useRuntimeConfig()
  return proxyRequest(event, `${config.bastionBackendUrl}/api/ingest/upload`)
})
