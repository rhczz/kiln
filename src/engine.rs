use anyhow::Context;
use std::path::Path;

const DEFAULT_LAYOUT: &str = include_str!("defaults/layout.html");
const DEFAULT_HOME: &str = include_str!("defaults/home.html");
const DEFAULT_POST: &str = include_str!("defaults/post.html");
const DEFAULT_PAGE: &str = include_str!("defaults/page.html");

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

        // Register the final names: external templates override defaults
        // Tera handles this automatically — if a template with the same name
        // was added later (from the external dir), it takes precedence.
        Ok(Self { tera })
    }

    pub fn render(&self, template: &str, context: &tera::Context) -> anyhow::Result<String> {
        self.tera
            .render(template, context)
            .with_context(|| format!("Failed to render template '{}'", template))
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;

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
}
