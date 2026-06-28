interface SettingsPayload {
  llmProvider?: string
  llmKey?: string
  llmModel?: string
  embedProvider?: string
  embedKey?: string
  embedModel?: string
}

export default defineEventHandler(async (event) => {
  const body = await readBody<SettingsPayload>(event)
  return proxyToBackend(event, '/api/config', {
    method: 'POST',
    body: {
      llm_provider: body.llmProvider || null,
      llm_key: body.llmKey || null,
      llm_model: body.llmModel || null,
      embed_provider: body.embedProvider || null,
      embed_key: body.embedKey || null,
      embed_model: body.embedModel || null
    }
  })
})
