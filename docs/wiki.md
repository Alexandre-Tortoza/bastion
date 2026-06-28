# Bastion — Formato da Wiki

> Spec de formato para tudo dentro de `wiki/`: nomes de arquivo, frontmatter
> YAML, wikilinks, formatos de `index.md` e `log.md`, supersedição, propostas
> de consolidação e regras de autoria para o LLM.
>
> A razão de ser da wiki (por que markdown-on-disk, pipeline de ingestão,
> estratégia de busca) está em `knowledge-layer.md`. Este arquivo é o contrato
> de formato que implementadores e o LLM precisam para ler e escrever páginas
> corretamente.

---

## 1. Estrutura de Diretórios

```
wiki/
├── index.md              ← catálogo de conteúdo; lido antes de qualquer operação
├── log.md                ← registro cronológico append-only
├── papers/               ← uma página por artigo ingerido
├── concepts/             ← conceitos cross-referenciados
├── methods/              ← metodologias e técnicas
├── decisions/            ← decisões de pesquisa (ADR-style)
├── comparisons/          ← comparações entre abordagens
├── synthesis/            ← síntese do estado da arte
├── reviews/              ← sessões de revisão de artigo em LaTeX
├── _pending/             ← propostas de consolidação aguardando aprovação
└── _lint/                ← relatórios de lint (gerados automaticamente)
```

---

## 2. Nomes de Arquivo

Todos os arquivos: lowercase ASCII, hífens como separadores, extensão `.md`.

| Diretório | Padrão | Exemplo |
|---|---|---|
| `papers/` | `<primeiroautor>-<ano>-<slug>.md` | `vaswani-2017-attention.md` |
| `concepts/` | `<slug>.md` | `self-attention.md` |
| `methods/` | `<slug>.md` | `lora-fine-tuning.md` |
| `decisions/` | `<NNNN>-<slug>.md` (4 dígitos, zero-padded) | `0001-no-lora.md` |
| `comparisons/` | `<slug>.md` | `lora-vs-full-finetune.md` |
| `synthesis/` | `<slug>.md` | `transformers-state-of-art.md` |
| `reviews/` | `<slug>-<YYYY-MM-DD>.md` | `thesis-draft-2026-06-27.md` |
| `_pending/` | `consolidation-<YYYY-MM-DD>-<slug>.md` | `consolidation-2026-06-27-attention-merge.md` |
| `_lint/` | `report-<YYYY-MM-DD>.md` | `report-2026-06-27.md` |

O número sequencial em `decisions/` é gerado pelo Rust incrementando o maior
número existente. Não reutilizar números de decisões rejeitadas.

---

## 3. Frontmatter YAML

Toda página começa com frontmatter YAML delimitado por `---`. O Rust parseia
esse bloco para indexar no SQLite. Campos listados como **obrigatórios** devem
estar presentes para a página ser indexada corretamente.

### Campos base (todos os kinds)

```yaml
---
title: "Título da página"        # obrigatório: string
kind: "paper"                    # obrigatório: ver enum abaixo
tier: "semantic"                 # obrigatório: semantic | episodic | working
created_at: "2026-06-27"         # obrigatório: ISO date YYYY-MM-DD
updated_at: "2026-06-27"         # obrigatório: atualizado em cada escrita
pinned: false                    # opcional: boolean; true = imune a qualquer decay
tags: []                         # opcional: lista de strings
---
```

**Enum `kind`:** `paper | concept | method | decision | comparison | synthesis | review | consolidation-proposal`

**Tier padrão por kind:**

| Kind | Tier padrão |
|---|---|
| `concept`, `method`, `comparison`, `synthesis`, `decision` | `semantic` |
| `paper`, `review` | `episodic` |
| `consolidation-proposal` | `working` |

### Campos adicionais por kind

**`paper`:**
```yaml
authors: ["Vaswani, Ashish", "Shazeer, Noam"]   # obrigatório
year: 2017                                        # obrigatório
venue: "NeurIPS 2017"                             # opcional
arxiv: "1706.03762"                               # opcional, sem prefixo de URL
doi: "10.48550/arXiv.1706.03762"                  # opcional
status: "ingested"                                # ingested | reviewed | superseded
raw_path: "raw/papers/vaswani-2017-attention/"    # obrigatório: caminho relativo à raiz
```

**`decision`:**
```yaml
status: "proposed"               # proposed | accepted | superseded | rejected
date: "2026-06-27"               # obrigatório: data da decisão
related_papers: []               # lista de caminhos wiki sem .md, ex: "papers/vaswani-2017-attention"
superseded_by: null              # null ou caminho da decisão que a substituiu
```

**`comparison`:**
```yaml
methods_compared: []             # lista de caminhos wiki, ex: "methods/lora-fine-tuning"
conclusion: null                 # null ou caminho da abordagem recomendada
```

**`synthesis`:**
```yaml
covers: []                       # lista de caminhos wiki de tópicos cobertos
as_of: "2026-06-27"             # data da última atualização completa
```

**`review`:**
```yaml
latex_session: "2026-06-27"     # data da sessão de revisão
suggestions_total: 0            # total de sugestões geradas
suggestions_accepted: 0         # quantas foram aceitas
```

**`consolidation-proposal`:**
```yaml
status: "pending"               # pending | approved | rejected
proposed_at: "2026-06-27"
pages_affected: []              # lista de caminhos relativos à raiz, ex: "wiki/concepts/self-attention.md"
```

---

## 4. Wikilinks

Três formas, todas resolvidas pelo backend Rust ao ler uma página:

| Forma | Sintaxe | Comportamento |
|---|---|---|
| Básico | `[[papers/vaswani-2017-attention]]` | título lido do frontmatter |
| Labelado | `[[papers/vaswani-2017-attention\|Attention is All You Need]]` | rótulo customizado |
| Com âncora | `[[concepts/self-attention#definition]]` | título + âncora de seção |

**Regras:**
- Caminhos sempre relativos à raiz de `wiki/`
- Sem `/` inicial, sem extensão `.md`
- Nunca usar caminhos absolutos do filesystem ou URLs externas para referências internas
- Um wikilink quebrado (página alvo não existe) é renderizado como texto simples com indicador visual no UI
- Wikilinks dentro de blocos de código (\`\`\`) são tratados como texto literal — sem resolução

---

## 5. Formato de `index.md`

`index.md` é a primeira coisa que o LLM lê antes de qualquer operação na wiki.
É um catálogo plano, atualizado pelo LLM após cada ingestão.

**Header:**
```markdown
# Wiki Index
Last updated: 2026-06-27 | Total pages: 42
```

**Entrada por página (uma linha):**
```
- [[papers/vaswani-2017-attention|Vaswani et al., 2017]] — Arquitetura Transformer, multi-head attention (paper, 2017-06-12)
```

Formato: `- [[path|Title]] — <descrição de uma linha> (kind, updated_at)`

**Seções:**
```markdown
## Papers (N)
## Concepts (N)
## Methods (N)
## Decisions (N)
## Comparisons (N)
## Synthesis (N)
## Reviews (N)
```

Cada seção lista apenas páginas com status atual (não supersedidas). Páginas
supersedidas são omitidas do `index.md` mas permanecem em disco.

---

## 6. Formato de `log.md`

Append-only. Novas entradas são adicionadas no topo (data mais recente
primeiro). Cada entrada é um heading `##` seguido de bullets.

**Formato do heading (grep-parseable):**
```
## [YYYY-MM-DD] action | subject
```

Ações válidas: `ingest`, `update`, `decision`, `consolidate`, `lint`, `review`

```bash
# Parsear as últimas 5 entradas:
grep "^## \[" wiki/log.md | head -5
```

**Exemplo de entrada:**
```markdown
## [2026-06-27] ingest | Vaswani 2017 — Attention is All You Need

- Criado: [[papers/vaswani-2017-attention]]
- Atualizado: [[concepts/self-attention]] (Transformer adicionado como implementação canônica)
- Atualizado: [[synthesis/transformers-state-of-art]] (citado como trabalho fundacional)
- Atualizado: index.md e log.md
```

Máximo 5 bullets por entrada. Sem prosa no body. O heading `##` nunca deve ter
texto adicional além do formato `[YYYY-MM-DD] action | subject`.

---

## 7. Corpo das Páginas por Kind

Seções obrigatórias por tipo. O LLM deve seguir essa estrutura ao criar ou
atualizar uma página.

### `paper`

```markdown
## Summary
<Resumo em 3–5 parágrafos: problema, abordagem, resultados principais>

## Methodology
<Como o estudo foi conduzido: dataset, arquitetura, protocolo experimental>

## Results
<Métricas e comparações com estado da arte anterior>

## Limitations
<O que o artigo reconhece como limitação ou não cobre>

## Relevant Citations
<Citações exatas com número de página>

- (p. 4) "We propose a new simple network architecture, the Transformer, based
  solely on attention mechanisms" — afirmação fundacional sobre a arquitetura
- (p. 7) "Multi-Head Attention allows the model to jointly attend to information
  from different representation subspaces" — definição do mecanismo chave
```

### `decision`

Segue o formato ADR (Architecture Decision Record) puro:

```markdown
## Context
<Por que essa decisão precisava ser tomada; alternativas consideradas>

## Decision
<O que foi decidido e por quê>

## Consequences
<Implicações positivas e negativas da decisão>

## Related Papers
<Wikilinks para artigos que embasaram a decisão>
```

### `concept`

```markdown
## Definition
<Definição precisa do conceito>

## Where Used
<Em quais papers, métodos ou outros conceitos este conceito aparece>

## See Also
<Wikilinks para conceitos relacionados>
```

### `method`

```markdown
## Description
<O que é o método e como funciona>

## When to Use
<Condições em que este método é preferível a alternativas>

## Papers Using This Method
<Wikilinks para papers que empregam este método>
```

### `comparison`

```markdown
## Approaches
<Descrição das abordagens comparadas, cada uma em sub-seção>

## Results Summary
<Tabela ou lista de resultados comparativos com fonte>

## Conclusion
<Qual abordagem é preferida e em que condições>
```

### `synthesis`

```markdown
## State of the Art (as of YYYY-MM-DD)
<Síntese do que se sabe sobre o tópico>

## Key Papers
<Papers mais importantes, com wikilinks>

## Open Questions
<O que ainda não foi resolvido ou está em aberto>
```

### `review`

```markdown
## Context
<Qual versão do artigo foi revisada, data>

## Suggestions
<Lista numerada de sugestões geradas pelo LLM>

## Decided
<Para cada sugestão: Aceita / Descartada + razão brevíssima>
```

---

## 8. Supersedição

Nunca deletar páginas. Quando uma página é substituída por uma versão mais
atual:

**Na página ANTIGA**, adicionar imediatamente após o frontmatter:

```markdown
> **Superseded** por [[path/to-new-page|Novo Título]] em YYYY-MM-DD.
> Ver a nova página para informação atual.
```

Também atualizar o frontmatter com `status: superseded` (papers, decisions) ou
`superseded_by: "path/to-new-page"` (outros kinds).

**Na página NOVA**, adicionar após o frontmatter:

```markdown
> **Supersedes** [[path/to-old-page|Título Antigo]] (YYYY-MM-DD).
```

**Regra especial para `decisions/`:** Páginas de decisão nunca são supersedidas
sem aprovação explícita do usuário. O LLM deve criar uma proposta em `_pending/`
e aguardar aprovação antes de modificar qualquer arquivo em `decisions/`.

---

## 9. Propostas de Consolidação (`_pending/`)

Antes de aplicar uma consolidação, o LLM cria um arquivo de proposta. O usuário
aprova ou rejeita via UI antes de qualquer escrita nas páginas afetadas.

**Estrutura do arquivo de proposta:**

```markdown
---
title: "Merge de páginas de atenção"
kind: consolidation-proposal
tier: working
status: pending
proposed_at: "2026-06-27"
created_at: "2026-06-27"
updated_at: "2026-06-27"
pages_affected:
  - "wiki/concepts/self-attention.md"
  - "wiki/concepts/cross-attention.md"
pinned: false
---

## Justificativa
<Por que a consolidação é necessária, baseada em evidências da wiki>

## Mudanças Propostas

### wiki/concepts/self-attention.md
<Descrição do que muda: "Adicionar seção sobre contraste com cross-attention; remover definição duplicada">

### wiki/concepts/cross-attention.md
<Descrição do que muda>

## Novas Páginas (se aplicável)
<Lista de páginas a serem criadas como parte da consolidação>
```

**Ao aprovar:** O LLM aplica todas as mudanças, atualiza `status: approved`,
registra em `log.md`, e move o arquivo para `_pending/applied/`.

**Ao rejeitar:** Atualizar `status: rejected` e adicionar:
```markdown
## Motivo da Rejeição
<Razão fornecida pelo usuário>
```

---

## 10. Regras de Autoria do LLM

O LLM deve seguir estas regras em toda operação de escrita na wiki:

1. **Ler antes de escrever.** Sempre ler a página existente antes de qualquer
   atualização. Nunca sobrescrever cegamente.

2. **Atualizar `index.md` e `log.md` na mesma operação.** Toda escrita de
   página deve ser acompanhada de atualização nesses dois arquivos. Não commitar
   um sem o outro.

3. **Cross-links bidirecionais obrigatórios.** Ao criar um paper que introduz
   o conceito X, também atualizar `concepts/X.md` para linkar de volta para o
   paper. Ao criar uma comparison, linkar a partir de todas as páginas de método
   comparadas.

4. **Nunca deletar seção de página existente** sem criar uma versão supersedida
   primeiro.

5. **Nunca modificar `decisions/` diretamente.** Propor via `_pending/` e
   aguardar aprovação.

6. **Frontmatter antes do body.** Ambos devem ser válidos antes de commitar.
   Atualizar `updated_at` para a data atual em toda escrita.

7. **Um ingest toca múltiplas páginas.** A operação de ingestão de um artigo
   deve criar/atualizar: a página do paper, páginas de conceitos e métodos
   relacionados, comparações relevantes, `index.md`, `log.md`. Tipicamente 10–15
   arquivos. Ver `knowledge-layer.md §4`.

8. **Não inventar conteúdo.** Toda afirmação em uma página wiki deve ter
   evidência no artigo original ou em páginas existentes da wiki. Não especular
   sem base.

---

## 11. Citações com Âncora de Página PDF

Para citar um trecho específico de um artigo no corpo de qualquer página wiki:

**No corpo do texto:**
```markdown
Ver [[papers/vaswani-2017-attention|Vaswani et al., 2017]], equação 1 (p. 4)
```

**Na seção `## Relevant Citations` de páginas `paper`:**
```markdown
- (p. 4) "We propose a new simple network architecture, the Transformer,
  based solely on attention mechanisms" — afirmação fundacional
- (p. 7) "Multi-Head Attention allows the model to jointly attend to information
  from different representation subspaces" — definição do mecanismo central
```

O backend Rust deriva a URL do PDF a partir do campo `raw_path` do frontmatter
do paper:

```
/raw/papers/vaswani-2017-attention/original.pdf#page=7
```

O UI usa essa URL para abrir o `PdfViewer` na página correta quando o usuário
clica em "Ver p. 7".
