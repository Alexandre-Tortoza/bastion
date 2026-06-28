# Bastion — Camada de Conhecimento

> Como a wiki funciona, o que aprendemos do ai-memory, e as decisões de design
> para o pipeline de ingestão, busca, consolidação e memória de decisões.

---

## 1. O Princípio Central: Compilar, Não Recuperar

A abordagem padrão com LLMs e documentos é RAG: você indexa arquivos, e o LLM
reconstrói o conhecimento do zero a cada pergunta. O Bastion faz diferente.

Quando um artigo é ingerido, o LLM **não** apenas vetoriza o conteúdo para
busca posterior. Ele lê o artigo, extrai as ideias centrais, e as **integra na
wiki existente** — atualizando páginas de conceitos, comparações entre
abordagens, síntese do estado da arte, e registrando onde o novo artigo
contradiz ou avança o que já estava lá. O conhecimento é compilado uma vez e
mantido atualizado. A wiki é o artefato — não os embeddings.

Isso é o que Karpathy chama de "compile, don't retrieve": a wiki é um artefato
persistente e acumulativo. As referências cruzadas já estão lá. As contradições
já foram sinalizadas. A síntese já reflete tudo que foi lido.

---

## 2. Três Camadas de Dados

```
raw/                  ← fontes brutas, imutáveis (PDFs + markdown extraído)
wiki/                 ← wiki mantida pelo LLM (markdown versionado em git)
data/db.sqlite        ← índice derivado (FTS5 + embeddings), nunca fonte de verdade
```

O markdown em `wiki/` é a fonte de verdade. O SQLite é um índice derivado —
reconstruível a partir dos arquivos. Qualquer divergência resolve-se relendo o
disco.

---

## 3. Estrutura da Wiki

```
wiki/
├── index.md          ← catálogo de todas as páginas (atualizado a cada ingestão)
├── log.md            ← registro cronológico append-only
├── papers/           ← uma página por artigo ingerido
├── concepts/         ← conceitos cross-referenciados
├── methods/          ← metodologias e técnicas
├── decisions/        ← decisões de pesquisa (ADR-style)
├── comparisons/      ← comparações entre abordagens
├── synthesis/        ← síntese do estado da arte
├── reviews/          ← revisões de artigos em produção
└── _pending/         ← propostas de consolidação aguardando aprovação
```

### `index.md` e `log.md`

Dois arquivos especiais guiam o LLM na navegação da wiki:

- **`index.md`** — catálogo orientado a conteúdo. Cada página listada com link,
  resumo de uma linha e metadados. O LLM lê isso primeiro antes de qualquer
  busca. Em escala moderada (~100 artigos, ~centenas de páginas) isso é
  suficiente como estratégia de retrieval — sem necessidade de embeddings.

- **`log.md`** — registro cronológico append-only. Cada entrada começa com
  `## [YYYY-MM-DD] ação | título` para ser parseável com `grep`. Serve como
  audit trail e referência de "o que foi feito recentemente".

---

## 4. Pipeline de Ingestão

```
PDF enviado
  │
  ▼
[1] bastion-ingest (Rust CLI)
    PDF → Markdown estruturado
    raw/papers/<name>/original.pdf.md
  │
  ▼
[2] LLM: extração de notas estruturadas
    - Resumo, metodologia, resultados, limitações
    - Citações relevantes com número de página
    - raw/papers/<name>/extracted-notes.md
  │
  ▼
[3] LLM: integração na wiki (fan-out de escrita)
    Um artigo tipicamente toca 10–15 páginas:
    - Cria/atualiza wiki/papers/<name>.md
    - Atualiza páginas de conceitos relacionados
    - Atualiza comparações e síntese
    - Registra em wiki/log.md
    - Atualiza wiki/index.md
  │
  ▼
[4] Indexação
    - Embeddings gerados e armazenados no SQLite
    - FTS5 sincronizado via triggers
  │
  ▼
Wiki persistentemente atualizada
```

### Por que "fan-out de escrita"

O passo [3] é a diferença fundamental em relação ao RAG. Quando o LLM integra
um artigo, ele não apenas cria uma página nova — ele modifica páginas
existentes. Se o artigo apresenta um resultado que supera uma abordagem já
documentada, ele atualiza a comparação. Se introduz um conceito novo, cria a
página do conceito e linka a partir de todos os artigos que o mencionam. O
conhecimento se compõe.

---

## 5. Memória de Decisões (ADRs)

Decisões metodológicas são armazenadas em `wiki/decisions/` no formato ADR
(Architecture Decision Record). Antes de qualquer operação que envolva escolha
metodológica, o LLM consulta esse diretório.

```markdown
# 0001: Não explorar fine-tuning com LoRA

Status: Aceito
Data: 2026-06-27
Contexto: Decidindo entre fine-tuning completo e LoRA.
Decisão: Fine-tuning completo — dataset pequeno, LoRA adicionaria
  complexidade desnecessária.
Consequências: Mais parâmetros treinados, maior custo, melhor controle.
Artigos relacionados: [[attention-is-all-you-need]], [[lora-paper]]
```

### Quando o LLM deve consultar as decisões

- **Chat**: antes de responder perguntas que envolvam escolhas metodológicas
- **Revisão de artigo**: verificar se sugestões contradizem decisões registradas
- **Ingestão**: identificar se o novo artigo contradiz ou reforça decisões
- **Consolidação**: preservar decisões como páginas semânticas (nunca sujeitas
  a decay)

---

## 6. Busca e Recuperação

### Estratégia em camadas

```
1. index.md           ← lido primeiro, identifica páginas candidatas
2. FTS5               ← busca textual sobre wiki + notas extraídas
3. Embeddings         ← busca semântica (reranking dos top-N do FTS5)
4. RRF                ← Reciprocal Rank Fusion: merge FTS5 + embeddings
5. Fallback           ← se wiki não tem a informação, busca nas notas brutas
```

### Por que RRF (Reciprocal Rank Fusion)

O FTS5 é forte para termos exatos e siglas técnicas. Embeddings são fortes para
similaridade semântica ("o que sabemos sobre regularização" encontra L1, L2,
dropout mesmo sem essas palavras). O RRF combina os dois rankings sem precisar
calibrar pesos: cada resultado recebe `1/(rank + k)` e os scores são somados.
Isso é o que o ai-memory implementa em produção com bons resultados.

### Fallback para fontes brutas

Se a wiki compilada não contém a informação, o sistema faz fallback para as
notas brutas (`raw/papers/*/extracted-notes.md`). Isso captura detalhes que
podem ter sido omitidos na síntese. Lição do MemPalace: resumos compilados às
vezes descartam detalhes que a pergunta exata precisa. O fallback é o escape
hatch.

### Âncoras de página no PDF

Para cada trecho retornado, o sistema mantém mapeamento para a página do PDF
original. O chat pode citar: "este resultado vem de [Paper X], página 7" com
link para o PDF na âncora da página correta.

---

## 7. Consolidação: Quando e Como

A consolidação é o processo de reescrever/sintetizar páginas da wiki após
acumulação de conhecimento. Não é automática por padrão — **deve ser
solicitada explicitamente pelo usuário**.

### Por que não automática

Aprendizado do ai-memory: consolidação automática não solicitada pode reescrever
páginas semânticas importantes (como decisões registradas) com sínteses menos
precisas. O custo de uma consolidação equivocada é alto — decisões de pesquisa
que somem da wiki.

### Quando o usuário deve solicitar consolidação

| Momento | O que o LLM faz |
|---|---|
| Após ingerir 5+ artigos relacionados | Sintetiza o estado da arte na área |
| Após registrar múltiplas decisões em um tema | Consolida em uma página de comparação |
| Quando páginas de conceitos se tornaram redundantes | Merge e supersedição |
| Quando a síntese geral ficou desatualizada | Reescreve `wiki/synthesis/` |

### Propostas de consolidação (pending)

Antes de aplicar uma consolidação, o LLM cria uma proposta em
`wiki/_pending/consolidation-<id>.md` com:
- Páginas que serão modificadas
- Diff do conteúdo proposto
- Justificativa baseada em evidências

O usuário aprova ou rejeita. Isso evita consolidações silenciosas que destroem
conhecimento duro de construir.

### Supersedição (não deleção)

Quando uma página é atualizada por consolidação, a versão anterior não é
deletada — ela é marcada como supersedida com link para a nova versão. O git
mantém o histórico completo. Páginas de `wiki/decisions/` nunca são
supersedidas automaticamente — apenas com aprovação explícita.

---

## 8. Embeddings

### Providers suportados

| Provider | Modelo padrão | Dimensões |
|---|---|---|
| OpenAI | text-embedding-3-small | 1536 |
| Voyage | voyage-3 | 1024 |
| Google / Gemini | gemini-embedding-001 | 768 |

### Contrato de vetores

Cada embedding no SQLite armazena `(provider, model, dim, content_sha256)`. Se
o provider/modelo mudar, os vetores antigos são ignorados até que um backfill
re-embeda as páginas. Nunca usar vetores stale de um modelo diferente — isso
produz buscas incorretas.

O backfill pode ser solicitado explicitamente via interface ou rodado em
background. O status de cobertura dos embeddings é visível no dashboard.

### Quando embeddings não são necessários

Para wikis pequenas (~50 artigos, ~200 páginas), o `index.md` + FTS5 é
suficiente e mais rápido. Embeddings se tornam críticos quando:
- Perguntas são altamente semânticas (não keywords exatas)
- A wiki tem >100 artigos e o index.md sozinho não é suficiente para filtrar
- A revisão de artigo precisa encontrar paralelos não óbvios

---

## 9. Tiers de Memória

Adaptado do modelo do ai-memory para o contexto de pesquisa acadêmica:

| Tier | O que armazena | Política |
|---|---|---|
| **Semântico** | Conceitos, sínteses, métodos consolidados | Indefinido — só supersedível por consolidação explícita |
| **Decisões** | ADRs em `wiki/decisions/` | Indefinido — nunca decay, aprovação para qualquer mudança |
| **Papers** | Páginas de artigos ingeridos | Indefinido enquanto o artigo existir |
| **Episódico** | Sessões de revisão, comparações pontuais | Decay suave após 180 dias sem acesso |
| **Working** | Contexto da sessão atual | Descartado ao final da sessão |

Páginas de decisões e papers são efetivamente "pinadas" — imunes a qualquer
política de decay.

---

## 10. Lint da Wiki

O lint verifica periodicamente a saúde da wiki. Deve ser solicitado pelo
usuário ou rodado em manutenção agendada.

### O que verificar

| Check | Por quê |
|---|---|
| Contradições entre páginas | Dois artigos relatam resultados opostos sem síntese |
| Claims desatualizados | Um artigo mais novo supera um resultado documentado |
| Páginas órfãs | Sem links de entrada, provavelmente perdida |
| Conceitos sem página própria | Mencionados mas não documentados |
| Cross-references quebradas | Links para páginas deletadas ou renomeadas |
| Decisões sem artigos relacionados | Decisão registrada mas sem embasamento linkado |

### Saída do lint

O resultado vai para `wiki/_lint/report-<date>.md` com:
- Lista de issues por severidade
- Para cada issue: páginas envolvidas e sugestão de ação
- Sem modificações automáticas — apenas relatório

---

## 11. O Que Não Fazer (Lições do ai-memory)

| Tentação | Por quê não |
|---|---|
| Usar embeddings como fonte de verdade | Vectors são índice, não conteúdo. Se o markdown sumir, o knowledge se vai. |
| Consolidação automática sem aprovação | Pode reescrever decisões importantes silenciosamente |
| FTS5 como única estratégia de busca | Falha em perguntas semânticas sem keywords exatas |
| Muitas ferramentas MCP/API | Consome contexto e confunde o LLM — manter superfície estreita |
| Injeção ampla de contexto automática | Token costs e contexto stale superam conveniência |
| Decay de páginas de decisões | Decisões metodológicas são permanentes |
| Deletar páginas supersedidas | Sempre manter histórico — git + supersedição |
| Multi-store (vector DB separado + SQLite + files) | Bugs de sync concentram-se nas costuras entre stores |

---

## 12. Referências

- [LLM Wiki — Karpathy](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)
- [ai-memory — ARCHITECTURE.md](../ai-memory/docs/ARCHITECTURE.md)
- [ai-memory — prior-art-implementation-findings.md](../ai-memory/docs/prior-art-implementation-findings.md)
- [ai-memory — research-karpathy-llm-wiki.md](../ai-memory/docs/research-karpathy-llm-wiki.md)
- [ai-memory — auto-improvement-loop.md](../ai-memory/docs/auto-improvement-loop.md)
