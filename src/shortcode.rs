use std::collections::HashMap;

use anyhow::Context;

use crate::engine::Engine;

#[derive(Debug, Clone)]
pub struct Shortcode {
    pub name: String,
    pub params: HashMap<String, String>,
    pub body: Option<String>,
}

/// Extract shortcodes from raw markdown, replacing them with HTML comment placeholders.
/// Returns (processed_markdown, extracted_shortcodes).
pub fn preprocess(md: &str) -> (String, Vec<Shortcode>) {
    let mut result = String::with_capacity(md.len());
    let mut shortcodes = Vec::new();
    let mut pos = 0;
    let bytes = md.as_bytes();

    while pos < md.len() {
        if matches_tag(bytes, pos, b"{{<") {
            if let Some((tag_end, name, params)) = parse_opening_tag(md, pos) {
                let close_tag = format!("{{{{< /{} >}}}}", name);
                if let Some(close_pos) = md[tag_end..].find(&close_tag) {
                    // Block shortcode
                    let body = md[tag_end..tag_end + close_pos].trim().to_string();
                    let full_end = tag_end + close_pos + close_tag.len();
                    shortcodes.push(Shortcode {
                        name,
                        params,
                        body: Some(body),
                    });
                    result.push_str(&format!("\n<!--KILN_SC_{}-->\n", shortcodes.len() - 1));
                    pos = full_end;
                } else {
                    // Inline shortcode
                    shortcodes.push(Shortcode {
                        name,
                        params,
                        body: None,
                    });
                    result.push_str(&format!("<!--KILN_SC_{}-->", shortcodes.len() - 1));
                    pos = tag_end;
                }
            } else {
                let next = next_char_boundary(md, pos);
                result.push_str(&md[pos..next]);
                pos = next;
            }
        } else {
            let next = next_char_boundary(md, pos);
            result.push_str(&md[pos..next]);
            pos = next;
        }
    }

    (result, shortcodes)
}

/// Replace shortcode placeholders in HTML with rendered template output.
pub fn postprocess(
    html: &str,
    shortcodes: &[Shortcode],
    engine: &Engine,
) -> anyhow::Result<String> {
    let mut result = html.to_string();

    for (i, sc) in shortcodes.iter().enumerate() {
        let placeholder = format!("<!--KILN_SC_{}-->", i);
        let template_name = format!("shortcodes/{}.html", sc.name);

        if !engine.template_exists(&template_name) {
            anyhow::bail!("missing shortcode template: {}", template_name);
        }

        let mut ctx = tera::Context::new();
        for (key, value) in &sc.params {
            ctx.insert(key, value);
        }
        if let Some(body) = &sc.body {
            ctx.insert("content", body);
            // Also provide rendered HTML for templates that want it
            let body_html = crate::render::markdown_to_html(body);
            ctx.insert("content_html", &body_html.html);
        }

        let rendered = engine
            .render(&template_name, &ctx)
            .with_context(|| format!("failed to render shortcode template: {}", template_name))?;
        result = result.replace(&placeholder, &rendered);
    }

    Ok(result)
}

fn matches_tag(bytes: &[u8], pos: usize, tag: &[u8]) -> bool {
    bytes[pos..].starts_with(tag)
}

fn next_char_boundary(s: &str, pos: usize) -> usize {
    s[pos..]
        .chars()
        .next()
        .map(|ch| pos + ch.len_utf8())
        .unwrap_or(s.len())
}

fn parse_opening_tag(md: &str, start: usize) -> Option<(usize, String, HashMap<String, String>)> {
    let rest = &md[start + 3..];
    let end = rest.find(">}}")?;
    let inner = rest[..end].trim();
    if inner.starts_with('/') {
        return None;
    }
    let (name, params) = parse_tag_content(inner);
    if name.is_empty() {
        return None;
    }
    Some((start + 3 + end + 3, name, params))
}

fn parse_tag_content(s: &str) -> (String, HashMap<String, String>) {
    let mut iter = s.splitn(2, |c: char| c.is_whitespace());
    let name = iter.next().unwrap_or("").to_string();
    let rest = iter.next().unwrap_or("");
    (name, parse_params(rest))
}

fn parse_params(s: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    let mut pos = 0;
    let chars: Vec<char> = s.chars().collect();

    while pos < chars.len() {
        // Skip whitespace
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }
        if pos >= chars.len() {
            break;
        }

        // Read key
        let key_start = pos;
        while pos < chars.len() && chars[pos] != '=' && !chars[pos].is_whitespace() {
            pos += 1;
        }
        let key: String = chars[key_start..pos].iter().collect();

        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }

        if pos < chars.len() && chars[pos] == '=' {
            pos += 1;
            while pos < chars.len() && chars[pos].is_whitespace() {
                pos += 1;
            }

            let value = if pos < chars.len() && chars[pos] == '"' {
                pos += 1;
                let val_start = pos;
                while pos < chars.len() && chars[pos] != '"' {
                    pos += 1;
                }
                let v: String = chars[val_start..pos].iter().collect();
                if pos < chars.len() {
                    pos += 1;
                }
                v
            } else {
                let val_start = pos;
                while pos < chars.len() && !chars[pos].is_whitespace() {
                    pos += 1;
                }
                chars[val_start..pos].iter().collect()
            };

            if !key.is_empty() {
                params.insert(key, value);
            }
        }
    }

    params
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_inline_shortcode() {
        let (md, scs) = preprocess("Hello {{< note text=\"hi\" >}} world");
        assert_eq!(scs.len(), 1);
        assert_eq!(scs[0].name, "note");
        assert_eq!(scs[0].params["text"], "hi");
        assert!(scs[0].body.is_none());
        assert!(md.contains("<!--KILN_SC_0-->"));
    }

    #[test]
    fn extracts_block_shortcode() {
        let (md, scs) = preprocess("Before\n{{< note >}}\nInner content\n{{< /note >}}\nAfter");
        assert_eq!(scs.len(), 1);
        assert_eq!(scs[0].name, "note");
        assert_eq!(scs[0].body.as_deref(), Some("Inner content"));
        assert!(md.contains("<!--KILN_SC_0-->"));
        assert!(md.contains("Before"));
        assert!(md.contains("After"));
    }

    #[test]
    fn extracts_multiple_shortcodes() {
        let (md, scs) = preprocess("{{< a >}}{{< b x=\"1\" >}}");
        assert_eq!(scs.len(), 2);
        assert_eq!(scs[0].name, "a");
        assert_eq!(scs[1].name, "b");
        assert_eq!(scs[1].params["x"], "1");
        assert!(md.contains("<!--KILN_SC_0-->"));
        assert!(md.contains("<!--KILN_SC_1-->"));
    }

    #[test]
    fn parses_multiple_params() {
        let (_, scs) = preprocess("{{< figure src=\"/img.png\" alt=\"test\" width=\"100\" >}}");
        assert_eq!(scs[0].params["src"], "/img.png");
        assert_eq!(scs[0].params["alt"], "test");
        assert_eq!(scs[0].params["width"], "100");
    }

    #[test]
    fn leaves_plain_text_untouched() {
        let (md, scs) = preprocess("Just some text with no shortcodes.");
        assert_eq!(scs.len(), 0);
        assert_eq!(md, "Just some text with no shortcodes.");
    }

    #[test]
    fn handles_nested_curly_braces() {
        let (md, scs) = preprocess("Some {{ code }} and {{< note >}}");
        assert_eq!(scs.len(), 1);
        assert!(md.contains("{{ code }}"));
    }

    #[test]
    fn preserves_utf8_without_shortcodes() {
        let input = "中文内容和 emoji 😀";
        let (md, scs) = preprocess(input);
        assert!(scs.is_empty());
        assert_eq!(md, input);
    }

    #[test]
    fn missing_shortcode_template_returns_error() {
        let engine = Engine::init(std::path::Path::new("/missing/templates")).unwrap();
        let shortcodes = vec![Shortcode {
            name: "callout".into(),
            params: HashMap::new(),
            body: None,
        }];

        let err = postprocess("<!--KILN_SC_0-->", &shortcodes, &engine).unwrap_err();
        assert!(err.to_string().contains("missing shortcode template"));
    }
}
