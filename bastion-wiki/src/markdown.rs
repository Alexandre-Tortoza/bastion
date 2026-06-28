//! YAML-frontmatter aware markdown parser and emitter.
//!
//! Parsing goes through `serde_yaml` directly (not `gray_matter`) to avoid
//! key-ordering and comment-loss bugs on round-trip. Adapted from ai-memory-wiki.

use std::collections::BTreeSet;

use crate::error::WikiResult;
use crate::types::WikiLink;

/// A parsed markdown document with detached frontmatter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Markdown {
    /// Frontmatter as JSON. `Null` when the source had no frontmatter.
    pub frontmatter: serde_json::Value,
    /// Body excluding the frontmatter block (and the closing `---\n`).
    pub body: String,
}

/// Parse markdown text into a [`Markdown`] struct.
///
/// Recognises the canonical `---\n<yaml>\n---\n` block at the very start.
/// Anything else is treated as body with `Null` frontmatter.
///
/// # Errors
/// Returns [`WikiError::Yaml`] if the frontmatter block exists but fails
/// to parse as YAML.
pub fn parse(input: &str) -> WikiResult<Markdown> {
    let trimmed = input.strip_prefix('\u{FEFF}').unwrap_or(input);
    if let Some(rest) = trimmed.strip_prefix("---\n")
        && let Some(end) = rest.find("\n---\n")
    {
        let fm_str = &rest[..end];
        let body = rest[end + 5..].to_string();
        let fm_yaml: serde_yaml::Value = serde_yaml::from_str(fm_str)?;
        let fm_json: serde_json::Value = serde_json::to_value(fm_yaml)?;
        return Ok(Markdown {
            frontmatter: fm_json,
            body,
        });
    }
    Ok(Markdown {
        frontmatter: serde_json::Value::Null,
        body: input.to_string(),
    })
}

/// Emit a [`Markdown`] back to a string.
///
/// Frontmatter is serialised through `serde_yaml`; a `Null` or empty-object
/// frontmatter is omitted entirely.
///
/// # Errors
/// Returns [`WikiError::Yaml`] if frontmatter cannot be serialised.
pub fn emit(md: &Markdown) -> WikiResult<String> {
    let has_fm = match &md.frontmatter {
        serde_json::Value::Null => false,
        serde_json::Value::Object(m) => !m.is_empty(),
        _ => true,
    };
    let mut out = String::with_capacity(md.body.len() + 64);
    if has_fm {
        let yaml = serde_yaml::to_string(&md.frontmatter)?;
        out.push_str("---\n");
        out.push_str(&yaml);
        if !yaml.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("---\n");
    }
    out.push_str(&md.body);
    Ok(out)
}

/// Derive a page title.
///
/// Priority: `frontmatter.title` → first `# ` heading in body → path stem.
#[must_use]
pub fn derive_title(frontmatter: &serde_json::Value, body: &str, path_str: &str) -> String {
    if let Some(t) = frontmatter.get("title").and_then(serde_json::Value::as_str) {
        let trimmed = t.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("# ") {
            let trimmed = rest.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    let stem = path_str.rsplit_once('/').map_or(path_str, |(_, n)| n);
    stem.strip_suffix(".md").unwrap_or(stem).to_string()
}

/// Extract wikilinks from a markdown body.
///
/// Supported forms:
/// - `[[path]]` — bare link
/// - `[[path|label]]` — labelled link
/// - `[[path#anchor]]` — anchored link
/// - `[[path#anchor|label]]` — anchored + labelled
///
/// Links inside fenced code blocks (``` or ~~~) are ignored.
/// External URLs (containing `://`) are ignored.
#[must_use]
pub fn extract_links(body: &str) -> Vec<WikiLink> {
    // Use BTreeSet keyed by (path, label, anchor) for deduplication + stable order.
    let mut seen: BTreeSet<(String, Option<String>, Option<String>)> = BTreeSet::new();
    let mut in_fence = false;

    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        extract_wikilinks_from_line(line, &mut seen);
    }

    seen.into_iter()
        .map(|(path, label, anchor)| WikiLink {
            path,
            label,
            anchor,
        })
        .collect()
}

fn extract_wikilinks_from_line(
    line: &str,
    out: &mut BTreeSet<(String, Option<String>, Option<String>)>,
) {
    let mut rest = line;
    while let Some(start) = rest.find("[[") {
        let after_open = &rest[start + 2..];
        let Some(close) = after_open.find("]]") else {
            break;
        };
        let raw = &after_open[..close];

        // Split label: `path#anchor|label` or `path|label`
        let (path_and_anchor, label) = if let Some((p, l)) = raw.split_once('|') {
            (p.trim(), Some(l.trim().to_string()))
        } else {
            (raw.trim(), None)
        };

        // Split anchor: `path#anchor`
        let (path, anchor) = if let Some((p, a)) = path_and_anchor.split_once('#') {
            (p.trim(), Some(a.trim().to_string()))
        } else {
            (path_and_anchor, None)
        };

        if !path.is_empty() && !path.contains("://") {
            out.insert((path.to_string(), label, anchor));
        }

        rest = &after_open[close + 2..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_round_trip() {
        let src = "---\ntitle: Hello\nkind: paper\n---\nBody text.\n";
        let md = parse(src).unwrap();
        assert_eq!(md.frontmatter["title"], "Hello");
        assert_eq!(md.frontmatter["kind"], "paper");
        assert_eq!(md.body, "Body text.\n");

        let emitted = emit(&md).unwrap();
        let re_parsed = parse(&emitted).unwrap();
        assert_eq!(re_parsed.frontmatter["title"], "Hello");
        assert_eq!(re_parsed.body, "Body text.\n");
    }

    #[test]
    fn parse_no_frontmatter() {
        let src = "Just a body.\n";
        let md = parse(src).unwrap();
        assert!(md.frontmatter.is_null());
        assert_eq!(md.body, src);
    }

    #[test]
    fn parse_bom_prefix() {
        let src = "\u{FEFF}---\ntitle: BOM Test\n---\nBody\n";
        let md = parse(src).unwrap();
        assert_eq!(md.frontmatter["title"], "BOM Test");
    }

    #[test]
    fn unterminated_frontmatter_is_body() {
        let src = "---\ntitle: No close\nBody continues\n";
        let md = parse(src).unwrap();
        assert!(md.frontmatter.is_null());
        assert_eq!(md.body, src);
    }

    #[test]
    fn emit_omits_empty_frontmatter() {
        let md = Markdown {
            frontmatter: serde_json::Value::Object(serde_json::Map::new()),
            body: "Hello\n".into(),
        };
        assert_eq!(emit(&md).unwrap(), "Hello\n");
    }

    #[test]
    fn derive_title_priority() {
        let path = "papers/vaswani-2017-attention.md";
        // Frontmatter wins
        let fm = serde_json::json!({ "title": "Attention Is All You Need" });
        assert_eq!(
            derive_title(&fm, "# Ignored\n", path),
            "Attention Is All You Need"
        );
        // H1 heading wins over stem
        assert_eq!(
            derive_title(&serde_json::Value::Null, "# From Heading\n", path),
            "From Heading"
        );
        // Stem fallback
        assert_eq!(
            derive_title(&serde_json::Value::Null, "no heading", path),
            "vaswani-2017-attention"
        );
    }

    #[test]
    fn extract_links_bare() {
        let links = extract_links("See [[papers/vaswani-2017-attention]] here.");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].path, "papers/vaswani-2017-attention");
        assert!(links[0].label.is_none());
        assert!(links[0].anchor.is_none());
    }

    #[test]
    fn extract_links_with_label() {
        let links = extract_links("[[papers/vaswani-2017-attention|Vaswani et al., 2017]]");
        assert_eq!(links[0].label.as_deref(), Some("Vaswani et al., 2017"));
    }

    #[test]
    fn extract_links_with_anchor() {
        let links = extract_links("[[concepts/self-attention#definition]]");
        assert_eq!(links[0].path, "concepts/self-attention");
        assert_eq!(links[0].anchor.as_deref(), Some("definition"));
    }

    #[test]
    fn extract_links_with_anchor_and_label() {
        let links =
            extract_links("[[concepts/self-attention#definition|Self-Attention Definition]]");
        assert_eq!(links[0].path, "concepts/self-attention");
        assert_eq!(links[0].anchor.as_deref(), Some("definition"));
        assert_eq!(links[0].label.as_deref(), Some("Self-Attention Definition"));
    }

    #[test]
    fn extract_links_ignores_fenced_code() {
        let body = "```\n[[papers/ignored]]\n```\n[[concepts/kept]]\n";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].path, "concepts/kept");
    }

    #[test]
    fn extract_links_ignores_urls() {
        let body = "[[https://example.com]] [[concepts/real]]";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].path, "concepts/real");
    }

    #[test]
    fn extract_links_deduplicates() {
        let body = "[[concepts/x]] and [[concepts/x]] again";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
    }
}
