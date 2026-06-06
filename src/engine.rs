use anyhow::Context;
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
}

impl Engine {
    pub fn init(templates_dir: &Path) -> anyhow::Result<Self> {
        let mut tera = tera::Tera::default();

        // Load embedded defaults first
        tera.add_raw_template("layout.html", DEFAULT_LAYOUT)?;
        tera.add_raw_template("home.html", DEFAULT_HOME)?;
        tera.add_raw_template("post.html", DEFAULT_POST)?;
        tera.add_raw_template("page.html", DEFAULT_PAGE)?;
        tera.add_raw_template("section.html", DEFAULT_SECTION)?;
        tera.add_raw_template("taxonomy.html", DEFAULT_TAXONOMY)?;
        tera.add_raw_template("term.html", DEFAULT_TERM)?;
        tera.add_raw_template("404.html", DEFAULT_404)?;

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
                tera.add_template_file(&path, Some(&name))
                    .with_context(|| format!("Failed to load template {:?}", path))?;
            }
        }

        Ok(Self { tera })
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
        assert!(html.contains("&lt;Title&gt; · &lt;Site&gt;"));
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
}
