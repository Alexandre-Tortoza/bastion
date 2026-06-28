# Bastion — Arquitetura Geral e Configuração

> Referência de sistema: dois processos, workspace Rust, variáveis de ambiente,
> providers LLM/embeddings, sequência de startup, autenticação, git e rotas da API.
> O modelo de dados, busca e estratégia de embeddings estão em `knowledge-layer.md`.
> A spec do frontend está em `interface.md`.

---

## 1. Dois Processos

O runtime do Bastion é composto por exatamente dois processos:

```
Browser
  │
  │ HTTP / SSE / WebSocket
  ▼
Nitro (Nuxt 3) — porta 3000
  │  assets, SSR, proxy HTTP
  │  server routes finas (sem lógica de negócio)
  │
  │ HTTP / SSE (internal)
  ▼
axum (Rust) — porta 8080
  │  toda a lógica: wiki, ingest, chat, review
  │  SQLite, markdown, git, subprocess
  ▼
wiki/ (markdown + git)
data/db.sqlite
raw/ (PDFs + notas brutas)
```

O browser **nunca** fala diretamente com o backend Rust. Toda autenticação,
CORS e roteamento de URL é responsabilidade da camada Nitro.

---

## 2. Rust Workspace

O workspace vive em `app/Cargo.toml` com crates em `app/crates/`:

| Crate | Responsabilidade |
|---|---|
| `bastion-core` | Tipos base, enums de erro, newtypes de ID — zero IO |
| `bastion-store` | SQLite + WAL, migrações (refinery), FTS5, armazenamento de embeddings |
| `bastion-wiki` | CRUD de markdown, watcher, resolução de wikilinks, commits git2 |
| `bastion-ingest` | Binário CLI `bastion-ingest`: PDF → Markdown |
| `bastion-llm` | Traits `LlmProvider` + `Embedder` e implementações (OpenAI, Anthropic, Voyage) |
| `bastion-web` | Router axum, handlers HTTP, SSE/WebSocket |
| `bastion-review` | Engine de revisão LaTeX contra a wiki |

Sem dependências circulares. A ordem de dependência: `core` ← `store` ← `wiki` ← `llm` ← `review` ← `web`.

---

## 3. Variáveis de Ambiente

Arquivo `.env` na raiz do projeto. O Rust lê via `dotenvy` no startup; o Nitro
lê via `runtimeConfig`. Startup falha imediatamente com mensagem clara se uma
variável obrigatória estiver ausente.

| Variável | Obrigatória | Padrão | Quem lê | Descrição |
|---|---|---|---|---|
| `BASTION_BACKEND_URL` | sim | `http://localhost:8080` | Nitro | URL interna do Rust |
| `BASTION_BACKEND_PORT` | não | `8080` | Rust | Porta do axum |
| `BASTION_API_TOKEN` | sim | — | ambos | Bearer token estático |
| `BASTION_WIKI_PATH` | sim | — | Rust | Caminho absoluto para `wiki/` |
| `BASTION_RAW_PATH` | sim | — | Rust | Caminho absoluto para `raw/` |
| `BASTION_DB_PATH` | sim | — | Rust | Caminho absoluto para `data/db.sqlite` |
| `BASTION_LLM_PROVIDER` | não | — | Rust | `openai` ou `anthropic` |
| `BASTION_LLM_MODEL` | não | — | Rust | Ex: `claude-sonnet-4-6`, `gpt-4o` |
| `BASTION_EMBED_PROVIDER` | não | — | Rust | `openai` ou `voyage` |
| `BASTION_EMBED_MODEL` | não | — | Rust | Ex: `text-embedding-3-small`, `voyage-3` |
| `OPENAI_API_KEY` | condicional | — | Rust | Obrigatória se algum provider for `openai` |
| `ANTHROPIC_API_KEY` | condicional | — | Rust | Obrigatória se LLM provider for `anthropic` |
| `VOYAGE_API_KEY` | condicional | — | Rust | Obrigatória se embed provider for `voyage` |
| `BASTION_GIT_AUTHOR_NAME` | não | `Bastion` | Rust | Autor dos commits da wiki |
| `BASTION_GIT_AUTHOR_EMAIL` | não | `bastion@local` | Rust | Email dos commits da wiki |

Se `BASTION_LLM_PROVIDER` não estiver configurado, todas as operações que
dependem do LLM retornam erro `503 LLM_NOT_CONFIGURED`. O sistema funciona
normalmente para leitura da wiki, busca FTS5 e embeddings (se configurados).

---

## 4. Trait `LlmProvider`

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(
        &self,
        messages: Vec<Message>,
        opts: ChatOptions,
    ) -> Result<String, LlmError>;

    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        opts: ChatOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, LlmError>> + Send>>, LlmError>;
}
```

Duas implementações: `OpenAiProvider` e `AnthropicProvider`. Selecionadas no
startup a partir de `BASTION_LLM_PROVIDER`. Sem switching em runtime.

**Outputs estruturados**: via JSON schema nativo de cada provider. OpenAI usa
`response_format: { type: "json_schema", json_schema: {...} }`. Anthropic usa
tool-use com schema tipado. Sem XML, sem wrapper Instructor-style — os erros de
serialização silenciosos de wrappers genéricos são um vetor de bugs documentado.

**Operações que invocam o LLM:**

| Operação | Quando |
|---|---|
| Extração de notas | Step [2] do pipeline de ingestão |
| Integração na wiki | Step [3] do pipeline de ingestão (fan-out) |
| Síntese do chat | Para cada query no chat |
| Análise de revisão | Quando o usuário clica "Revisar" no editor |
| Consolidação | Apenas sob demanda explícita (ver `knowledge-layer.md §7`) |

---

## 5. Trait `Embedder`

```rust
#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbedError>;
    fn dimensions(&self) -> usize;
    fn provider_id(&self) -> &str;
    fn model_id(&self) -> &str;
}
```

| Implementação | Provider | Modelo padrão | Dimensões |
|---|---|---|---|
| `OpenAiEmbedder` | `openai` | `text-embedding-3-small` | 1536 |
| `VoyageEmbedder` | `voyage` | `voyage-3` | 1024 |

Contrato de vetores: cada embedding no SQLite armazena `(provider, model, dim,
content_sha256)`. Ver `knowledge-layer.md §8` para a política completa de
backfill e detecção de stale vectors.

---

## 6. Sequência de Startup

```
1. Carregar config (.env + env)
   └─ Falhar se variáveis obrigatórias ausentes (mensagem inclui quais)

2. Abrir SQLite (WAL mode, foreign_keys=ON)
   └─ Rodar migrações pendentes via refinery

3. Verificar BASTION_WIKI_PATH
   └─ Criar subdiretórios padrão se não existem:
      papers/ concepts/ methods/ decisions/ comparisons/ synthesis/ reviews/
      _pending/ _lint/

4. Inicializar repo git em BASTION_WIKI_PATH
   └─ git init se .git/ não existe
   └─ Configurar author com BASTION_GIT_AUTHOR_NAME/EMAIL

5. Criar wiki/index.md e wiki/log.md com stubs se não existem

6. Sincronizar índice SQLite
   └─ Varrer arquivos wiki, comparar mtime vs. last_indexed_at no SQLite
   └─ Re-parsear frontmatter e atualizar FTS5 apenas para arquivos alterados

7. Verificar embeddings
   └─ Se provider/modelo configurado difere do armazenado: log warning + enfileirar backfill
   └─ Backfill roda em background; não bloqueia startup

8. Iniciar axum em BASTION_BACKEND_PORT
```

---

## 7. Autenticação

Token estático único. Sem usuários, sem sessões no backend Rust.

**Rust (axum middleware):**
- Valida `Authorization: Bearer <token>` em todas as rotas exceto `/api/health`
- Retorna `401 UNAUTHORIZED` se header ausente ou token inválido
- Middleware aplicado globalmente via `axum::middleware::from_fn`

**Nitro (server middleware `server/middleware/auth.ts`):**
- Lê cookie `bastion_token` (HTTP-only, set em `/api/auth/login`)
- Injeta `Authorization: Bearer <token>` em todo `fetch()` para o Rust
- Redireciona para `/login` se cookie ausente, exceto rotas `/api/auth/*` e `/login`

**Login (`POST /api/auth/login` — rota Nitro, não proxied):**
- Body: `{ "token": "..." }`
- Valida contra `BASTION_API_TOKEN` do `runtimeConfig`
- Sucesso: seta cookie HTTP-only `bastion_token` + retorna `{ "ok": true }`
- Falha: `401 { "error": "Invalid token" }`

---

## 8. Subprocess `bastion-ingest`

O binário `bastion-ingest` é invocado pelo `bastion-web` como subprocess para
isolar o parsing de PDF do processo principal.

**Invocação:**
```
bastion-ingest --input <caminho-absoluto-pdf> --output-dir <raw/papers/<name>/>
```

**Sucesso (exit 0):**
- Escreve `original.md` no output-dir
- Imprime JSON no stdout: `{"output_path": "<abs>", "page_count": <N>}`

**Falha (exit != 0):**
- Imprime mensagem de erro no stderr

**Exit codes:**

| Código | Significado |
|---|---|
| 0 | Sucesso |
| 1 | PDF inválido ou formato não suportado |
| 2 | Erro de IO (permissão, disco cheio) |
| 3 | Argumentos inválidos |

O handler `bastion-web` lê stdout como JSON para obter o caminho de saída,
loga stderr independentemente do exit code, e retorna erro `422` para o cliente
se exit != 0.

---

## 9. Integração Git

Cada escrita na wiki produz um commit imediato. Crate `git2`.

**Formato do commit:**
```
<action>(<scope>): <subject>
```

**Actions válidas:**

| Action | Quando |
|---|---|
| `ingest` | Paper adicionado à wiki |
| `update` | Atualização de página existente |
| `consolidate` | Consolidação de conhecimento |
| `decision` | Criação ou atualização de ADR |
| `lint` | Relatório de lint gerado |
| `review` | Sessão de revisão LaTeX registrada |

**Exemplos:**
```
ingest(papers): add vaswani-2017-attention
update(concepts): self-attention after vaswani-2017
decision(0003): reject lora fine-tuning
consolidate(synthesis): transformer-survey updated
```

Branch único `main`. Sem feature branches. O author/email vem de
`BASTION_GIT_AUTHOR_NAME` e `BASTION_GIT_AUTHOR_EMAIL`.

---

## 10. Rotas da API Rust

Todas as rotas montadas sob `/api`. O Nitro espelha esses caminhos exatamente
nos proxies (ver `interface.md §6`).

| Método | Rota | Handler | Notas |
|---|---|---|---|
| GET | `/api/wiki/pages` | `bastion-web` | Query params: `kind`, `recent`, `sort`, `limit` |
| GET | `/api/wiki/pages/*path` | `bastion-web` | `path` é o caminho relativo a `wiki/` sem `.md` |
| GET | `/api/wiki/decisions` | `bastion-web` | Query param: `status` |
| GET | `/api/wiki/log` | `bastion-web` | Query param: `limit` (default 10) |
| POST | `/api/wiki/pending` | `bastion-web` | Cria proposta em `_pending/` |
| POST | `/api/chat/query` | `bastion-web` | SSE streaming — `Content-Type: text/event-stream` |
| POST | `/api/ingest/upload` | `bastion-web` | Multipart form-data com campo `file` |
| GET | `/api/ingest/status/:jobId` | `bastion-web` | Polling de status do job |
| POST | `/api/review/analyze` | `bastion-web` | Body: `{ "latex": "..." }` |
| POST | `/api/auth/login` | *não existe no Rust* | Tratado no Nitro, sem proxy |
| GET | `/api/health` | `bastion-web` | Sem auth middleware |

**Respostas de erro (todos os endpoints):**
```json
{ "error": "Mensagem legível para o usuário", "code": "MACHINE_CODE_SNAKE_CASE" }
```

Códigos HTTP: 400 (bad request), 401 (unauthorized), 404 (not found), 409
(conflict), 422 (unprocessable), 500 (server error), 503 (provider not configured).

**Erros em SSE (chat):**
```
event: error
data: {"error": "LLM provider error: rate limit exceeded"}

```
O evento `error` é enviado antes de fechar o stream. O cliente deve sempre
ouvir este evento.
