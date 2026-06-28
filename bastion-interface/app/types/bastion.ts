export type WikiPageKind = 'paper' | 'concept' | 'method' | 'result' | 'strategy' | 'decision' | 'comparison' | 'synthesis' | 'review' | 'consolidation-proposal'

export interface WikiPage {
  path: string
  title: string
  kind: WikiPageKind
  tier: 'semantic' | 'episodic' | 'working'
  created_at?: string
  updated_at?: string
  pinned: boolean
  tags?: string[]
  status?: string
  frontmatter?: Record<string, unknown>
  body?: string
  wikilinks?: WikiLink[]
  backlinks?: WikiLink[]
}

export interface WikiLink {
  path: string
  title?: string
  label?: string
  anchor?: string
}

export interface WikiGraphLink {
  source: string
  target: string
  label?: string
  anchor?: string
}

export interface WikiGraph {
  pages: WikiPage[]
  links: WikiGraphLink[]
}

export interface Decision extends WikiPage {
  kind: 'decision'
  decision_status: 'proposed' | 'accepted' | 'superseded' | 'rejected'
  date?: string
  context_excerpt?: string
  related_papers?: string[]
}

export interface SearchHit {
  path: string
  title: string
  kind?: string
  snippet: string
}

export interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  refs?: Reference[]
  timestamp: string
  streaming?: boolean
}

export interface Reference {
  kind: 'paper' | 'wiki'
  title: string
  excerpt: string
  page?: number
  pdf_url?: string
  wiki_path?: string
}

export interface Suggestion {
  id: string
  location: string
  found_in_wiki: string
  suggested_change: string
  wiki_ref: string
  severity: 'info' | 'warning' | 'error'
}

export interface IngestStatus {
  job_id: string
  step: 'received' | 'converting' | 'extracting' | 'integrating' | 'indexed' | 'embedding'
  done: boolean
  error?: string
  wiki_path?: string
}
