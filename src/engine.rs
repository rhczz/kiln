use anyhow::Context;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use crate::model::PageKind;

const DEFAULT_LAYOUT: &str = include_str!("defaults/layout.html");
const DEFAULT_HOME: &str = include_str!("defaults/home.html");
const DEFAULT_POST: &str = include_str!("defaults/post.html");
const DEFAULT_PAGE: &str = include_str!("defaults/page.html");
const DEFAULT_SECTION: &str = include_str!("defaults/section.html");
const DEFAULT_TAXONOMY: &str = include_str!("defaults/taxonomy.html");
const DEFAULT_TERM: &str = include_str!("defaults/term.html");
const DEFAULT_404: &str = include_str!("defaults/404.html");

pub struct Engine {
    tera: tera::Tera,
    template_sources: HashMap<String, String>,
    asset_mappings: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, String>>>,
}

impl Engine {
    pub fn init(templates_dir: &Path) -> anyhow::Result<Self> {
        let mut tera = tera::Tera::default();
        let mut template_sources = HashMap::new();

        // Load embedded defaults first
        let defaults: [(&str, &str); 8] = [
            ("layout.html", DEFAULT_LAYOUT),
            ("home.html", DEFAULT_HOME),
            ("post.html", DEFAULT_POST),
            ("page.html", DEFAULT_PAGE),
            ("section.html", DEFAULT_SECTION),
            ("taxonomy.html", DEFAULT_TAXONOMY),
            ("term.html", DEFAULT_TERM),
            ("404.html", DEFAULT_404),
        ];
        for (name, source) in defaults {
            tera.add_raw_template(name, source)?;
            template_sources.insert(name.to_string(), source.to_string());
        }

        // If external templates directory exists, load and override
        if templates_dir.is_dir() {
            let pattern = templates_dir.join("**").join("*.html");
            let pattern_str = pattern.to_string_lossy();
            for entry in glob::glob(&pattern_str)? {
                let path = entry?;
                let name = path
                    .strip_prefix(templates_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_string();
                let source = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read template {:?}", path))?;
                // Use add_template_file for correct relative path resolution in Tera
                tera.add_template_file(&path, Some(&name))
                    .with_context(|| format!("Failed to load template {:?}", path))?;
                template_sources.insert(name, source);
            }
        }

        let asset_mappings =
            std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()));
        register_asset_url_fn(&mut tera, asset_mappings.clone())?;

        Ok(Self {
            tera,
            template_sources,
            asset_mappings,
        })
    }

    pub fn update_asset_mappings(&self, mappings: std::collections::HashMap<String, String>) {
        if let Ok(mut guard) = self.asset_mappings.write() {
            *guard = mappings;
        }
    }

    pub fn render(&self, template: &str, context: &tera::Context) -> anyhow::Result<String> {
        self.tera.render(template, context).map_err(|e| {
            let stack = vec![crate::TemplateFrame {
                template: template.to_string(),
                line: None,
            }];
            let mut msg = format!("Failed to render template '{}': {}", template, e);
            if let Some(source) = std::error::Error::source(&e) {
                msg.push_str(&format!("\n  caused by: {}", source));
            }
            let diag = crate::Diagnostic::error(std::path::PathBuf::from(template), msg)
                .with_template_stack(stack);
            anyhow::anyhow!("{}", diag)
        })
    }

    pub fn template_exists(&self, name: &str) -> bool {
        self.tera.get_template(name).is_ok()
    }

    pub fn resolve_template(&self, kind: &PageKind, collection: Option<&str>) -> String {
        let candidates = match kind {
            PageKind::Single => {
                // Single pages use the collection's configured template
                // (already resolved by the caller), no lookup needed
                return collection
                    .map(|s| format!("{}.html", s))
                    .unwrap_or_else(|| "page.html".into());
            }
            PageKind::Alias => return String::new(),
            PageKind::Home => return "home.html".into(),
            PageKind::NotFound => return "404.html".into(),
            PageKind::Section => {
                let mut c = Vec::new();
                if let Some(col) = collection {
                    c.push(format!("{}_section.html", col));
                }
                c.push("section.html".into());
                c.push("list.html".into());
                c
            }
            PageKind::TaxonomyIndex => {
                vec!["taxonomy.html".into(), "list.html".into()]
            }
            PageKind::Term => {
                let mut c = Vec::new();
                if let Some(tax) = collection {
                    c.push(format!("{}_term.html", tax));
                }
                c.push("term.html".into());
                c.push("list.html".into());
                c
            }
            PageKind::Paginate => {
                vec!["paginate.html".into(), "list.html".into()]
            }
        };

        candidates
            .into_iter()
            .find(|name| self.template_exists(name))
            .unwrap_or_else(|| "list.html".into())
    }

    /// Returns the source of a registered template, if available.
    pub fn template_source(&self, name: &str) -> Option<&str> {
        self.template_sources.get(name).map(|s| s.as_str())
    }

    /// Returns all registered template names.
    pub fn template_names(&self) -> Vec<&str> {
        self.template_sources.keys().map(|s| s.as_str()).collect()
    }

    /// Mutable access to the underlying Tera for registering runtime functions.
    /// Returns shared access to template sources, used for parallel rendering
    /// where each thread builds its own Tera instance.
    pub fn shared_template_sources(
        &self,
    ) -> std::sync::Arc<std::collections::HashMap<String, String>> {
        std::sync::Arc::new(self.template_sources.clone())
    }

    /// Creates an Engine from a pre-built Tera instance (no template loading).
    /// Used in parallel rendering tasks that build their own Tera from shared sources.
    pub fn init_tera_only(tera: tera::Tera) -> Self {
        Self {
            tera,
            template_sources: HashMap::new(),
            asset_mappings: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    /// Returns the full transitive dependency chain for a template
    /// (direct extends/include + recursive). Uses BFS with dedup.
    pub fn template_deps(&self, template_name: &str) -> Vec<String> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        queue.push_back(template_name.to_string());

        while let Some(current) = queue.pop_front() {
            if !visited.insert(current.clone()) {
                continue;
            }
            result.push(current.clone());

            if let Some(source) = self.template_sources.get(&current) {
                for dep in extract_template_deps(source) {
                    if !visited.contains(&dep) {
                        queue.push_back(dep);
                    }
                }
            }
        }

        result
    }
}

/// Register `asset_url(path)` as a Tera global function.
/// After registration, templates can use `{{ asset_url(path="style.css") }}`.
pub fn register_asset_url_fn(
    tera: &mut tera::Tera,
    mappings: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, String>>>,
) -> tera::Result<()> {
    tera.register_function(
        "asset_url",
        move |args: &std::collections::HashMap<String, tera::Value>| {
            // Support both asset_url("style.css") and asset_url(path="style.css")
            let path = args
                .get("path")
                .or_else(|| args.get("0"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let normalized = path.trim_start_matches("./").trim_start_matches('/');
            let guard = mappings
                .read()
                .map_err(|e| tera::Error::msg(e.to_string()))?;
            let resolved = guard
                .get(normalized)
                .cloned()
                .unwrap_or_else(|| path.to_string());
            Ok(tera::Value::String(resolved))
        },
    );
    Ok(())
}

/// Extract template names from `{% extends "..." %}`, `{% include "..." %}`,
/// and `{% import "..." %}` directives using simple string scanning.
fn extract_template_deps(source: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let mut pos = 0;

    while let Some(tag_start) = source[pos..].find("{%") {
        let abs = pos + tag_start;
        let tag_content = &source[abs..];

        let tag_end = match tag_content.find("%}") {
            Some(end) => end,
            None => break,
        };

        // Strip whitespace control characters from tag body: {%- ... -%}
        let inner = tag_content[2..tag_end].trim_matches(|c: char| c.is_whitespace() || c == '-');

        // Check for extends, include, import, or from
        let directive = if inner.starts_with("extends") {
            "extends"
        } else if inner.starts_with("include") {
            "include"
        } else if inner.starts_with("import") {
            "import"
        } else if inner.starts_with("from") {
            "from"
        } else {
            pos = abs + tag_end + 2;
            continue;
        };

        // Find the quoted string after the directive
        let after_directive = &inner[directive.len()..].trim_start();
        let quote = match after_directive.chars().next() {
            Some(c @ ('"' | '\'')) => c,
            _ => {
                pos = abs + tag_end + 2;
                continue;
            }
        };

        // Extract the path between matching quotes
        if let Some(inner_end) = after_directive[1..].find(quote) {
            let dep_name = &after_directive[1..=inner_end];
            let dep = if dep_name.ends_with(".html") {
                dep_name.to_string()
            } else {
                format!("{}.html", dep_name)
            };
            deps.push(dep);
        }

        pos = abs + tag_end + 2;
    }

    deps
}

#[cfg(test)]
mod tests {
    use super::{Engine, PageKind};

    #[test]
    fn escapes_template_variables_but_preserves_safe_body() {
        let engine = Engine::init(std::path::Path::new("/missing/templates")).unwrap();
        let mut ctx = tera::Context::new();
        ctx.insert(
            "config",
            &serde_json::json!({
                "language": "en",
                "title": "<Site>",
                "subtitle": "",
                "description": "",
                "base_url": "https://example.com",
                "stylesheet_href": "/assets/styles.css",
                "author": {"name": "A&B"},
                "nav": [{"label": "<Nav>", "href": "/"}],
                "footer_links": [],
            }),
        );
        ctx.insert(
            "site",
            &serde_json::json!({
                "language": "en",
                "title": "<Site>",
                "subtitle": "",
                "description": "",
                "base_url": "https://example.com",
                "stylesheet_href": "/assets/styles.css",
                "author": {"name": "A&B"},
            }),
        );
        ctx.insert(
            "theme",
            &serde_json::json!({
                "intro": "Intro",
                "email": "hi@example.com",
                "nav": [{"label": "<Nav>", "href": "/"}],
                "footer_links": [{"label": "GitHub", "href": "https://example.com"}],
                "home_image": {
                    "enabled": true,
                    "src": "/hero.jpg",
                    "alt": "Hero",
                    "width": 1,
                    "height": 1,
                },
            }),
        );
        ctx.insert("title", "<Title>");
        ctx.insert("description", "A&B");
        ctx.insert("body", "<main><strong>safe</strong></main>");
        ctx.insert("path", "");
        ctx.insert("og_type", "website");

        let html = engine.render("layout.html", &ctx).unwrap();
        assert!(html.contains("&lt;Title&gt; - &lt;Site&gt;"));
        assert!(html.contains("A&amp;B"));
        assert!(html.contains("&lt;Nav&gt;"));
        assert!(html.contains("<main><strong>safe</strong></main>"));
    }

    #[test]
    fn resolves_home_to_home_template() {
        let engine = Engine::init(std::path::Path::new("/missing/templates")).unwrap();
        assert_eq!(engine.resolve_template(&PageKind::Home, None), "home.html");
    }

    #[test]
    fn resolves_section_to_section_template() {
        let engine = Engine::init(std::path::Path::new("/missing/templates")).unwrap();
        assert_eq!(
            engine.resolve_template(&PageKind::Section, None),
            "section.html"
        );
    }

    #[test]
    fn resolves_collection_specific_section() {
        let engine = Engine::init(std::path::Path::new("/missing/templates")).unwrap();
        // posts_section.html doesn't exist, so it falls back to section.html
        assert_eq!(
            engine.resolve_template(&PageKind::Section, Some("posts")),
            "section.html"
        );
    }

    #[test]
    fn template_exists_checks_correctly() {
        let engine = Engine::init(std::path::Path::new("/missing/templates")).unwrap();
        assert!(engine.template_exists("home.html"));
        assert!(engine.template_exists("section.html"));
        assert!(!engine.template_exists("nonexistent.html"));
    }

    // ── template dependency extraction ──

    #[test]
    fn extract_extends_directive() {
        let source = r#"{% extends "layout.html" %}<html></html>"#;
        let deps = super::extract_template_deps(source);
        assert_eq!(deps, vec!["layout.html"]);
    }

    #[test]
    fn extract_include_directive() {
        let source = r#"{% include "partials/header.html" %}<body></body>"#;
        let deps = super::extract_template_deps(source);
        assert_eq!(deps, vec!["partials/header.html"]);
    }

    #[test]
    fn extract_appends_html_suffix() {
        let source = r#"{% extends "base" %}"#;
        let deps = super::extract_template_deps(source);
        assert_eq!(deps, vec!["base.html"]);
    }

    #[test]
    fn extract_handles_whitespace_control() {
        let source = r#"{%- extends "layout.html" -%}"#;
        let deps = super::extract_template_deps(source);
        assert_eq!(deps, vec!["layout.html"]);
    }

    #[test]
    fn extract_single_quotes() {
        let source = "{% include 'partial.html' %}";
        let deps = super::extract_template_deps(source);
        assert_eq!(deps, vec!["partial.html"]);
    }

    #[test]
    fn extract_multiple_deps() {
        let source = r#"
            {% extends "layout.html" %}
            {% include "header.html" %}
            {% include "footer.html" %}
        "#;
        let deps = super::extract_template_deps(source);
        assert_eq!(deps, vec!["layout.html", "header.html", "footer.html"]);
    }

    #[test]
    fn extract_import_directive() {
        let source = r#"{% import "macros.html" as macros %}"#;
        let deps = super::extract_template_deps(source);
        assert_eq!(deps, vec!["macros.html"]);
    }

    #[test]
    fn extract_ignores_other_tags() {
        let source = r#"{% if true %}{% endif %}text{% include "partial.html" %}"#;
        let deps = super::extract_template_deps(source);
        assert_eq!(deps, vec!["partial.html"]);
    }

    #[test]
    fn template_source_returns_saved_source() {
        let engine = Engine::init(std::path::Path::new("/missing/templates")).unwrap();
        let source = engine.template_source("home.html");
        assert!(source.is_some());
        assert!(source.unwrap().contains("{{ body }}") || source.unwrap().contains("archive"));
    }

    #[test]
    fn template_names_returns_all_registered() {
        let engine = Engine::init(std::path::Path::new("/missing/templates")).unwrap();
        let names = engine.template_names();
        assert!(names.contains(&"home.html"));
        assert!(names.contains(&"layout.html"));
        assert!(names.contains(&"post.html"));
        assert!(names.contains(&"404.html"));
        // At least 8 embedded defaults
        assert!(names.len() >= 8);
    }

    #[test]
    fn template_deps_follows_chain() {
        let dir = std::env::temp_dir().join("kiln_test_template_deps");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("partials")).unwrap();
        std::fs::create_dir_all(dir.join("posts")).unwrap();
        // layout.html includes header and footer
        std::fs::write(
            dir.join("layout.html"),
            r#"{% include "partials/header.html" %}{{ body }}{% include "partials/footer.html" %}"#,
        )
        .unwrap();
        std::fs::write(dir.join("partials/header.html"), "header").unwrap();
        std::fs::write(dir.join("partials/footer.html"), "footer").unwrap();
        // post.html extends layout.html
        std::fs::write(
            dir.join("posts/post.html"),
            r#"{% extends "layout.html" %}"#,
        )
        .unwrap();

        let engine = Engine::init(&dir).unwrap();
        let deps = engine.template_deps("posts/post.html");
        // should include: posts/post.html → layout.html → partials/header.html, partials/footer.html
        assert!(deps.contains(&"posts/post.html".to_string()));
        assert!(deps.contains(&"layout.html".to_string()));
        assert!(deps.contains(&"partials/header.html".to_string()));
        assert!(deps.contains(&"partials/footer.html".to_string()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn template_deps_no_extends_is_just_self() {
        let engine = Engine::init(std::path::Path::new("/missing/templates")).unwrap();
        // Default templates don't use extends/include
        let deps = engine.template_deps("home.html");
        assert_eq!(deps, vec!["home.html"]);
    }
}
