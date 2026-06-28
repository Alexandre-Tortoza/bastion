import type { H3Event } from 'h3'
import { createError, sendStream, setHeader } from 'h3'

interface BackendOptions {
  method?: 'GET' | 'POST' | 'PUT' | 'DELETE'
  body?: unknown
  query?: Record<string, string | number | boolean>
  stream?: boolean
}

export async function proxyToBackend(
  event: H3Event,
  path: string,
  opts: BackendOptions = {}
) {
  const config = useRuntimeConfig()
  const url = `${config.bastionBackendUrl}${path}`
  const headers: Record<string, string> = {
    'Authorization': `Bearer ${config.bastionApiToken}`,
    'Content-Type': 'application/json'
  }

  try {
    if (opts.stream) {
      const response = await fetch(url, {
        method: opts.method ?? 'GET',
        headers,
        body: opts.body ? JSON.stringify(opts.body) : undefined
      })

      if (!response.ok) {
        const error = await readBackendError(response)
        throw createError({
          statusCode: response.status,
          statusMessage: error.message,
          data: error
        })
      }

      setHeader(event, 'Content-Type', 'text/event-stream')
      setHeader(event, 'Cache-Control', 'no-cache')
      return sendStream(event, response.body!)
    }

    return await $fetch(url, {
      method: opts.method ?? 'GET',
      headers,
      body: opts.body as Record<string, unknown> | undefined,
      query: opts.query
    })
  } catch (error) {
    if (isH3Error(error)) throw error

    throw createError({
      statusCode: 503,
      statusMessage: 'Backend Bastion indisponível',
      data: {
        code: 'BACKEND_UNAVAILABLE',
        error: 'Backend Bastion indisponível'
      }
    })
  }
}

async function readBackendError(response: Response) {
  try {
    const payload = await response.json()
    return {
      code: payload.code ?? 'BACKEND_ERROR',
      error: payload.error ?? payload.message ?? response.statusText,
      message: payload.error ?? payload.message ?? response.statusText
    }
  } catch {
    return {
      code: 'BACKEND_ERROR',
      error: response.statusText,
      message: response.statusText
    }
  }
}

function isH3Error(error: unknown) {
  return typeof error === 'object' && error !== null && 'statusCode' in error
}
