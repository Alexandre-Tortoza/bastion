<p align="center">
  <img src=".assets/bastion-logo.png" width="160" alt="Bastion">
</p>

# Bastion — Wiki de Pesquisa Acadêmica com LLM

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)

> **compile, don't retrieve** — a wiki é um artefato persistente que acumula
> conhecimento a cada fonte ingerida e a cada pergunta respondida. O LLM escreve
> e mantém a wiki; você faz as perguntas certas.

---

## O Problema

Durante minha graduação, observei um padrão recorrente entre colegas que usavam LLMs para auxiliar na escrita de artigos acadêmicos: o modelo frequentemente trazia afirmações que não tinham respaldo nos papers que o pesquisador realmente tinha lido, inventava citações ou fazia deduções sem fundamento no corpus de referências do trabalho.

O problema não era o modelo em si — era que ele operava sem estar ancorado ao que o pesquisador de fato conhecia. Cada pergunta começava do zero, sem memória das fontes que o usuário havia acumulado.

---

## O que é LLM-Wiki

O conceito de **LLM-Wiki** foi proposto por [Andrej Karpathy](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) como alternativa ao RAG tradicional. A ideia central é simples:

Em vez de buscar documentos a cada query, o LLM **compila** um artefato de conhecimento — uma wiki em Markdown — e o mantém atualizado. Cada paper ingerido, cada decisão tomada, cada pergunta respondida **escreve** nessa wiki. Quando o modelo precisa responder ou sugerir algo, ele opera dentro desse artefato, não fora dele.

O resultado é um sistema que "sabe o que você sabe" — e nada além disso.

---

## A Solução — O que o Bastion faz

O Bastion é um protótipo que aplica o LLM-Wiki ao contexto de pesquisa acadêmica. O objetivo é criar um ambiente onde o pesquisador possa:

**1. Ingerir papers (PDF)**
O paper é convertido para texto, o LLM extrai notas estruturadas e cria páginas atômicas na wiki — conceitos, métodos, resultados e estratégias, cada um como uma nota própria com link de volta para o paper de origem.

**2. Chat ancorado na wiki**
Perguntas são respondidas apenas com base no que está na wiki. A busca combina FTS5 (textual) e embeddings (semântica) via Reciprocal Rank Fusion — sem invenções, com referência exata à fonte.

**3. Decisões de pesquisa (ADR-style)**
As escolhas metodológicas do pesquisador ficam registradas no formato Architecture Decision Record. Antes de qualquer sugestão, o modelo consulta essas decisões — e não propõe o que já foi descartado com razão.

**4. Revisão do artigo em LaTeX**
O pesquisador escreve o paper diretamente no editor LaTeX integrado. Ao solicitar revisão, o LLM lê o rascunho contra a wiki e aponta contradições, afirmações sem suporte nas referências e oportunidades de citação.

**5. Grafo de conhecimento**
Visualização das conexões entre as notas da wiki — papers, conceitos, métodos, resultados e estratégias como nós; wikilinks como arestas.

---

## Stack

**Backend**

| Tecnologia | Uso |
|---|---|
| Rust (2024 edition) | Linguagem principal do backend |
| axum + tokio | Servidor HTTP assíncrono |
| rusqlite + FTS5 | Banco de dados e busca textual |
| git2 | Versionamento automático da wiki |
| refinery | Migrações do banco |

**Frontend**

| Tecnologia | Uso |
|---|---|
| Nuxt 4 (Vue 3 + Nitro) | Framework frontend + proxy para o backend |
| Nuxt UI + Tailwind CSS 4 | Componentes e estilos |
| CodeMirror 6 + codemirror-lang-latex | Editor LaTeX integrado |
| Vue Flow + dagre | Grafo de conhecimento |
| marked | Renderização Markdown |

**LLM e Embeddings**

| Tecnologia | Uso |
|---|---|
| Anthropic, OpenAI, OpenRouter, Gemini | Providers LLM (configurável via `.env`) |
| OpenAI, Voyage, Gemini | Embeddings para busca semântica |

**Infraestrutura**

| Tecnologia | Uso |
|---|---|
| Docker + Docker Compose | Containers para produção e desenvolvimento |
| pdftotext (poppler) | Conversão PDF → texto |

---

## Inspirações

- [LLM Wiki — Andrej Karpathy](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) — conceito e filosofia que fundamenta o projeto
- [ai-memory — akitaonrails](https://github.com/akitaonrails/ai-memory) — referência de implementação

---

## Status: v0.0.1

Este é o primeiro protótipo funcional do Bastion. Algumas interfaces estão incompletas ou inconsistentes, e há partes do código que refletem mais a exploração do que decisões definitivas de design.

Decidi abrir o código quando o sistema atingiu um nível mínimo de usabilidade — o suficiente para demonstrar o conceito. Também funcionou como laboratório de aprendizado em tecnologias que não dominava, especialmente Rust e Nuxt 4.

É provável que uma v0.2 ou v1.0 venha a ser construída do zero, aproveitando os pontos fortes e corrigindo os fracos identificados nessa iteração. Por isso, contribuições, issues e feedback são bem-vindos — especialmente críticas ao design.

---

## Como Rodar

```bash
cp .env.example .env
# edite .env com seu provider LLM e caminhos desejados

docker compose up
# interface disponível em http://localhost:3000
```

Para desenvolvimento:

```bash
docker compose -f docker-compose.dev.yml up
```

Veja `.env.example` para todas as variáveis disponíveis (LLM provider, embeddings, caminhos de dados).
