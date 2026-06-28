import { readBody } from 'h3'

export default defineEventHandler(async (event) => {
  const body = await readBody(event)
  return proxyToBackend(event, '/api/review/analyze', { method: 'POST', body })
})
