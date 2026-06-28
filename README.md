# Bastion — Academic Research Wiki

Uma plataforma de pesquisa acadêmica baseada no padrão
[LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) com
memória persistente de decisões, artigos e referências.

> **compile, don't retrieve** — a wiki é um artefato persistente que acumula
> conhecimento a cada fonte ingerida e a cada pergunta respondida. O LLM escreve
> e mantém a wiki; você faz as perguntas certas.

---

## Visão Geral

Bastion integra quatro capacidades principais:

1. **Ingestão de artigos acadêmicos** — PDF/HTML → Markdown → LLM extrai notas
   estruturadas e integra na wiki (resumo, metodologia, resultados, limitações,
   citações com página)
2. **Chat com a wiki** — perguntas respondidas com trechos exatos e referência
   à página do PDF original (FTS5 + embeddings)
3. **Decisões de pesquisa** — armazenamento ADR-style do *porquê* de cada
   escolha metodológica, para que o LLM sempre considere o contexto completo
4. **Revisão inteligente de artigos** — o LLM lê o LaTeX em produção contra a
   wiki e aponta contradições, gaps, ou oportunidades

---

## Arquitetura

### Camadas

```
+----------------------------------------------------+
|                Nuxt 3 (Nitro)                      |
|  Frontend Vue 3 + server routes proxy              |
|  Nitro faz fetch() para o Rust backend             |
+--------------------+-------------------------------+
                     | HTTP / WebSocket
+--------------------v-------------------------------+
|          Rust + axum (backend da wiki)             |
|                                                     |
|  +----------+  +---------+  +---------+  +-------+  |
|  |  Ingest  |  |  Query  |  |  Review |  | Wiki  |  |
|  | Pipeline |  |  Engine |  |  Engine |  |  Mgr  |  |
|  +----+-----+  +----+----+  +----+----+  +---+---+  |
|       |             |            |           |       |
|  +----v-------------v------------v-----------v---+  |
|  |        SQLite + Markdown (git)                |  |
|  |   FTS5 | Embeddings | Páginas | Decisões      |  |
|  +------------------------------------------------+  |
+------------------------------------------------------+
```

### Stack

| Camada | Tecnologia |
|---|---|
| Frontend | Nuxt 3 (Vue 3 + Nitro) |
| Backend | Rust + axum + tokio |
| Banco | SQLite (rusqlite, FTS5, sqlite-vec) |
| Wiki | Markdown em disco + git2 (versionamento) |
| LLM | Traits genéricas (Anthropic, OpenAI, etc.) |
| Embeddings | OpenAI / Voyage / Google (via traits) |
| PDF→Markdown | `pdf-extract` / `lopdf` + pandoc |
| Editor LaTeX | CodeMirror 6 (vue-codemirror) |
| Chat | WebSocket |
| Estilo | Tailwind CSS + Nuxt UI |

### Fluxo de requisição

```
Browser (Vue) ──> Nuxt (Nitro) ──fetch()──> Rust (axum) ──> SQLite + Wiki
                     |                            ^
                     |  /api/wiki/*               |
                     |  /api/chat/query           |
                     +---> proxy p/ Rust (localhost:PORT)
```

Nitro **não** duplica lógica da wiki. Cada server route faz `fetch()` para o
Rust backend, que é a única fonte de verdade. Nuxt cuida só de SSR, layout,
autenticação (se houver) e roteamento do frontend.

---

## Estrutura de Diretórios

```
bastion/
├── AGENTS.md                  # Instruções canônicas para o LLM
├── index.md                   # Este arquivo
│
├── raw/                       # Fontes brutas (imutáveis)
│   ├── papers/
│   │   └── <paper-name>/
│   │       ├── original.pdf
│   │       ├── original.md         # PDF → Markdown (conversão)
│   │       └── extracted-notes.md  # Notas extraídas pelo LLM
│   └── assets/
│
├── wiki/                      # Wiki auto-mantida pelo LLM
│   ├── index.md               # Catálogo de tudo na wiki
│   ├── log.md                 # Registro cronológico
│   ├── papers/                # Página por artigo
│   ├── concepts/              # Conceitos cross-referenciados
│   ├── methods/               # Metodologias
│   ├── decisions/             # Decisões de pesquisa (ADR-style)
│   ├── comparisons/           # Comparações entre abordagens
│   ├── synthesis/             # Síntese do estado da arte
│   └── reviews/               # Revisões de artigos em produção
│
├── app/                       # Código da aplicação
│   ├── Cargo.toml             # Workspace Rust
│   ├── crates/
│   │   ├── bastion-core/      # Tipos, traits, IDs
│   │   ├── bastion-store/     # SQLite (FTS5, embeddings, índices)
│   │   ├── bastion-wiki/      # Operações na wiki (CRUD de páginas)
│   │   ├── bastion-ingest/    # Pipeline PDF → Markdown → Notas
│   │   ├── bastion-llm/       # Providers LLM + Embedders
│   │   ├── bastion-web/       # API server (axum + WebSocket)
│   │   └── bastion-review/    # Motor de revisão de artigos
│   │
│   └── frontend/              # Nuxt 3
│       ├── nuxt.config.ts
│       ├── package.json
│       ├── app.vue
│       ├── pages/
│       │   ├── index.vue          # Dashboard
│       │   ├── editor.vue         # Editor LaTeX
│       │   ├── chat.vue           # Chat
│       │   ├── wiki/
│       │   │   ├── index.vue      # Navegador wiki
│       │   │   ├── papers.vue     # Lista de artigos
│       │   │   └── decisions.vue  # Lista de decisões
│       │   ├── ingest.vue         # Upload de PDF
│       │   └── review.vue         # Revisão de artigo
│       ├── components/
│       │   ├── LaTeXEditor.vue
│       │   ├── ChatPanel.vue
│       │   ├── ReferencePanel.vue
│       │   ├── WikiTree.vue
│       │   ├── DecisionCard.vue
│       │   ├── PdfViewer.vue
│       │   └── SuggestionList.vue
│       ├── server/              # Nitro (proxy para Rust)
│       │   ├── api/
│       │   │   ├── wiki/
│       │   │   │   ├── pages.get.ts
│       │   │   │   ├── pages/[path].get.ts
│       │   │   │   └── decisions.get.ts
│       │   │   ├── chat/
│       │   │   │   └── query.post.ts
│       │   │   ├── ingest/
│       │   │   │   └── upload.post.ts
│       │   │   └── review/
│       │   │       └── analyze.post.ts
│       │   └── utils/
│       │       └── backend.ts   # Helper fetch() p/ Rust backend
│       └── composables/
│           └── useWiki.ts
│
└── data/                      # Dados runtime
    ├── db.sqlite              # SQLite (FTS5 + embeddings)
    └── models/                # Reservado para modelos locais
```

---

## Fluxos

### Ingestão

```
PDF enviado pelo usuário
  │
  ▼
[1] Nuxt: /api/ingest/upload → proxy → Rust POST /api/ingest
  │
  ▼
[2] bastion-ingest: converte PDF → Markdown (pandoc / pdf-extract)
  │  raw/papers/<name>/original.md
  │
  ▼
[3] bastion-llm: LLM extrai notas estruturadas
  │  raw/papers/<name>/extracted-notes.md
  │
  ▼
[4] bastion-wiki: LLM integra na wiki
  │  - Cria/atualiza wiki/papers/<name>.md
  │  - Atualiza páginas de conceitos, comparações, síntese
  │  - Atualiza wiki/index.md + log.md
  │
  ▼
[5] bastion-store: gera embeddings + índice FTS5
```

### Chat / Query

```
Pergunta no chat (Vue)
  │
  ▼
[1] Nuxt: POST /api/chat/query → proxy → Rust POST /api/query
  │
  ▼
[2] Busca combinada (Rust):
  │  - Lê wiki/index.md → páginas candidatas
  │  - FTS5 → matches textuais com snippet
  │  - Embeddings → similaridade semântica (RRF)
  │
  ▼
[3] LLM sintetiza resposta com citações exatas
  │  (artigo, seção, página)
  │
  ▼
Vue renderiza resposta + painel de referências
```

### Revisão de Artigo (LaTeX)

```
Usuário escreve no editor LaTeX
  │
  ▼
[1] Clica "Revisar" → Nuxt POST /api/review/analyze → Rust POST /api/review
  │
  ▼
[2] bastion-review (Rust):
  │  - Recebe o LaTeX + consulta a wiki
  │  - Busca decisões em wiki/decisions/
  │  - Busca artigos relacionados a cada afirmação
  │  - Identifica contradições
  │
  ▼
[3] LLM retorna sugestões estruturadas
  │
  ▼
[4] Usuário aceita/descarta cada sugestão
```

---

## Decisões de Pesquisa (ADRs)

Armazenadas em `wiki/decisions/` no formato Architecture Decision Record:

```markdown
# 0001: Não explorar fine-tuning com LoRA

Status: Aceito
Data: 2026-06-27
Contexto: Precisávamos decidir entre fine-tuning completo e LoRA
Decisão: Optamos por fine-tuning completo porque o dataset é pequeno
Consequências: Maior custo computacional, melhor controle
Artigos relacionados: [[attention-is-all-you-need]], [[lora-paper]]
```

Antes de qualquer operação com escolha metodológica, o LLM **deve** consultar
`wiki/decisions/` para não sugerir abordagens descartadas.

---

## Busca e Recuperação

1. **Índice** (`wiki/index.md`) — catálogo de páginas
2. **FTS5** — busca textual com snippet
3. **Embeddings** — reranking semântico
4. **RRF** — merge FTS5 + embeddings
5. **Fallback** — fontes brutas (`raw/`)

| Cenário | Busca |
|---|---|
| Pergunta factual | FTS5 + embeddings + índice |
| "O que sabemos sobre X?" | Índice + FTS5 |
| Sugestão de revisão | Embeddings + decisões |
| "Onde no PDF?" | Mapeamento trecho → página |

---

## Próximos Passos

### Fase 1 — Fundação
- [ ] Workspace Rust (cargo workspace + crates)
- [ ] `bastion-core`: tipos base
- [ ] `bastion-store`: SQLite schema + FTS5 + migrações
- [ ] `bastion-wiki`: CRUD markdown + git checkpoints
- [ ] `bastion-web`: server axum (API REST + WebSocket)
- [ ] Nuxt + Tailwind + rotas básicas + proxy para Rust
- [ ] AGENTS.md

### Fase 2 — Ingestão
- [ ] `bastion-ingest`: PDF → Markdown
- [ ] `bastion-llm`: traits para providers
- [ ] Extração de notas por LLM
- [ ] Integração com a wiki

### Fase 3 — Busca e Chat
- [ ] Embeddings
- [ ] FTS5 + RRF
- [ ] Chat (WebSocket)
- [ ] Frontend: Chat + Referências

### Fase 4 — Editor e Revisão
- [ ] Editor LaTeX (CodeMirror)
- [ ] `bastion-review`
- [ ] UI de sugestões

### Fase 5 — Decisões e Wiki
- [ ] CRUD de decisões (ADRs)
- [ ] Consulta automática de decisões
- [ ] Navegador wiki
- [ ] Lint da wiki

---

## Referências

- [LLM Wiki (Karpathy)](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)
- [ai-memory](https://github.com/akitaonrails/ai-memory)
- [qmd](https://github.com/tobi/qmd)
