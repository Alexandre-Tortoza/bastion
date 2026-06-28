import { execFile } from 'node:child_process'
import { access, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { promisify } from 'node:util'
import { createError, readBody, setHeader } from 'h3'

const execFileAsync = promisify(execFile)
const maxSourceBytes = 2 * 1024 * 1024
const compileTimeoutMs = 120_000

interface ExecError extends Error {
  code?: string
  signal?: string
  stdout?: string
  stderr?: string
}

function escapeLatexText(value: string) {
  return value.replace(/[\\{}#$%&_~^]/g, match => `\\${match}`)
}

function normalizeImagePath(value: string) {
  return value.trim().replace(/^['"]|['"]$/g, '')
}

async function fileExists(path: string) {
  try {
    await access(path)
    return true
  } catch {
    return false
  }
}

async function readLog(path: string) {
  try {
    return await readFile(path, 'utf8')
  } catch {
    return ''
  }
}

async function replaceMissingImages(source: string, workDir: string) {
  const includeGraphicsPattern = /\\includegraphics(\s*\[[^\]]*\])?\s*\{([^{}]+)\}/g
  const replacements = await Promise.all([...source.matchAll(includeGraphicsPattern)].map(async (match) => {
    const imagePath = normalizeImagePath(match[2] ?? '')
    const exists = imagePath && !imagePath.startsWith('/') && await fileExists(join(workDir, imagePath))

    if (exists) {
      return { original: match[0], replacement: match[0] }
    }

    const label = escapeLatexText(imagePath || 'unknown image')
    return {
      original: match[0],
      replacement: `\\fbox{\\parbox[c][35mm][c]{0.9\\linewidth}{\\centering Missing image: \\texttt{${label}}}}`
    }
  }))

  return replacements.reduce(
    (nextSource, item) => nextSource.replace(item.original, item.replacement),
    source
  )
}

function applyLatexCompatibilityFixes(source: string) {
  return source.replace(/\\usepackage\s*\[([^\]]*)\]\s*\{babel\}/g, (match, options: string) => {
    const nextOptions = options
      .split(',')
      .map(option => option.trim() === 'brazil' ? 'brazilian' : option.trim())
      .join(',')

    return match.replace(`[${options}]`, `[${nextOptions}]`)
  })
}

export default defineEventHandler(async (event) => {
  const body = await readBody<{ source?: string }>(event)
  const source = body?.source

  if (typeof source !== 'string' || !source.trim()) {
    throw createError({ statusCode: 400, statusMessage: 'LaTeX source is required' })
  }

  if (Buffer.byteLength(source, 'utf8') > maxSourceBytes) {
    throw createError({ statusCode: 413, statusMessage: 'LaTeX source is too large' })
  }

  const workDir = await mkdtemp(join(tmpdir(), 'bastion-latex-'))
  const outDir = join(workDir, 'out')
  const texPath = join(workDir, 'main.tex')
  const pdfPath = join(outDir, 'main.pdf')
  const logPath = join(outDir, 'main.log')

  try {
    await mkdir(outDir)
    const compatibleSource = applyLatexCompatibilityFixes(source)
    const compilableSource = await replaceMissingImages(compatibleSource, workDir)
    await writeFile(texPath, compilableSource, 'utf8')

    await execFileAsync('latexmk', [
      '-xelatex',
      '-interaction=nonstopmode',
      '-halt-on-error',
      '-file-line-error',
      '-outdir=out',
      'main.tex'
    ], {
      cwd: workDir,
      timeout: compileTimeoutMs,
      maxBuffer: 1024 * 1024 * 20
    })

    const pdf = await readFile(pdfPath)

    setHeader(event, 'Content-Type', 'application/pdf')
    setHeader(event, 'Content-Disposition', 'inline; filename="paper.pdf"')
    setHeader(event, 'Cache-Control', 'no-store')

    return pdf
  } catch (error) {
    const execError = error as ExecError
    const timedOut = execError.code === 'ETIMEDOUT' || execError.signal === 'SIGTERM'
    const log = await readLog(logPath)
    const details = [log, execError.stderr, execError.stdout, execError.message]
      .filter(Boolean)
      .map(item => item!.trim())
      .filter(Boolean)
      .join('\n\n')
    const message = timedOut
      ? `LaTeX compilation timed out after ${compileTimeoutMs / 1000}s.`
      : details || 'Unknown LaTeX compilation error'

    throw createError({
      statusCode: execError.code === 'ENOENT' ? 503 : timedOut ? 504 : 422,
      statusMessage: execError.code === 'ENOENT'
        ? 'latexmk is not installed'
        : timedOut
          ? 'LaTeX compilation timed out'
          : 'Failed to compile LaTeX',
      data: { message }
    })
  } finally {
    await rm(workDir, { recursive: true, force: true })
  }
})
