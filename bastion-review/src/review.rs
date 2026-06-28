//! LaTeX review engine — calls LLM, writes wiki page under `reviews/`.

use std::sync::Arc;

use bastion_core::{CommitAction, WikiPath, WritePageRequest};
use bastion_llm::{ChatOptions, LlmProvider, Message, Role};
use bastion_wiki::Wiki;
use chrono::Local;
use serde_json::json;
use tracing::info;

use crate::error::{ReviewError, ReviewResult};

pub struct ReviewOutput {
    pub wiki_path: String,
    pub suggestions_raw: String,
    pub suggestions_total: usize,
}

pub struct ReviewEngine {
    pub llm: Arc<dyn LlmProvider>,
    pub wiki: Arc<Wiki>,
}

impl ReviewEngine {
    pub async fn analyze_latex(
        &self,
        latex: &str,
        slug_hint: Option<&str>,
        wiki_context: Option<&str>,
    ) -> ReviewResult<ReviewOutput> {
        if latex.trim().is_empty() {
            return Err(ReviewError::Core("latex content is empty".into()));
        }

        let wiki_context = wiki_context.unwrap_or("Nenhum contexto recuperado da wiki.");
        let system = format!(
            "Você é um revisor acadêmico especializado em textos LaTeX com acesso à wiki Bastion.\n\
             Analise o paper em escrita e cruze ativamente as afirmações, lacunas e oportunidades com os papers e notas recuperados da wiki.\n\
             Use o contexto da wiki para sugerir referências, comparações, definições e melhorias mais assertivas.\n\
             Priorize sugestões concretas que melhorem clareza, estrutura, argumentação, base bibliográfica, matemática e estilo.\n\
             Quando usar uma fonte da wiki, cite o caminho da página entre parênteses.\n\
             Responda em Markdown, APENAS com uma lista numerada de sugestões, sem introdução ou conclusão.\n\
             Cada item deve indicar localização, tipo e sugestão concreta.\n\
             Exemplo:\n\
             1. [Introdução, §1] Referências: relacione a motivação com `papers/exemplo.md`. Sugestão: acrescente uma frase comparando ...\n\n\
             ## Contexto recuperado da wiki\n{wiki_context}"
        );

        let messages = vec![
            Message {
                role: Role::System,
                content: system,
            },
            Message {
                role: Role::User,
                content: latex.to_string(),
            },
        ];

        let suggestions_raw = self
            .llm
            .chat(
                messages,
                ChatOptions {
                    max_tokens: Some(4096),
                    ..Default::default()
                },
            )
            .await?;

        let suggestions_total = count_suggestions(&suggestions_raw);

        let today = Local::now().date_naive();
        let today_str = today.format("%Y-%m-%d").to_string();
        let base_slug = slug_hint.unwrap_or("review");
        let file_name = format!("{base_slug}-{today_str}.md");
        let wiki_path_str = format!("reviews/{file_name}");

        let path = WikiPath::new(&wiki_path_str).map_err(|e| ReviewError::Core(e.to_string()))?;

        let frontmatter = json!({
            "title": format!("Revisão: {base_slug}"),
            "kind": "review",
            "tier": "episodic",
            "latex_session": today_str,
            "suggestions_total": suggestions_total,
            "suggestions_accepted": 0,
            "created_at": today_str,
            "updated_at": today_str,
        });

        let body = format!(
            "## Context\nRevisão de LaTeX em {today_str}.\n\n\
             ## Suggestions\n{suggestions_raw}\n\n\
             ## Decided\n(a preencher pelo usuário)\n"
        );

        self.wiki.write_page(WritePageRequest {
            path: path.clone(),
            frontmatter,
            body,
            action: CommitAction::Review,
            scope: "reviews".into(),
            subject: format!("add {file_name}"),
        })?;

        info!(wiki_path = %wiki_path_str, suggestions_total, "review complete");

        Ok(ReviewOutput {
            wiki_path: wiki_path_str,
            suggestions_raw,
            suggestions_total,
        })
    }
}

fn count_suggestions(s: &str) -> usize {
    s.lines()
        .filter(|l| {
            let trimmed = l.trim_start();
            trimmed
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
                && trimmed.contains(". ")
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_suggestions_basic() {
        assert_eq!(count_suggestions("1. Foo\n2. Bar\n3. Baz"), 3);
    }

    #[test]
    fn count_suggestions_empty() {
        assert_eq!(count_suggestions(""), 0);
    }

    #[test]
    fn count_suggestions_no_items() {
        assert_eq!(count_suggestions("no numbered items here"), 0);
    }

    #[test]
    fn count_suggestions_mixed() {
        let s = "Some preamble\n1. First item\n2. Second item\nNot an item";
        assert_eq!(count_suggestions(s), 2);
    }
}
